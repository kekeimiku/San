use core::ops::Bound::Included;
use std::{
    collections::{BTreeMap, HashSet},
    ffi::OsStr,
    fs,
    fs::OpenOptions,
    io,
    io::{BufWriter, Write},
    path::PathBuf,
};

use argh::{FromArgValue, FromArgs};
use dumper::map::{encode_map_to_writer, Map};
use utils::consts::{Address, MAX_BUF_SIZE};

use crate::{
    b::{convert_bin_to_txt, load_pointer_map},
    e::PointerSeacher,
    utils::{select_module, Spinner},
};

pub struct Target(pub Address);

impl FromArgValue for Target {
    fn from_arg_value(value: &str) -> Result<Self, String> {
        let value = value.trim_start_matches("0x");
        let address = Address::from_str_radix(value, 16).map_err(|e| e.to_string())?;
        Ok(Self(address))
    }
}

pub struct Offset(pub (usize, usize));

impl FromArgValue for Offset {
    fn from_arg_value(value: &str) -> Result<Self, String> {
        let (lr, ur) = value.split_once(':').ok_or("err")?;
        let lr = lr.trim_start_matches('-').parse::<usize>().map_err(|e| e.to_string())?;
        let ur = ur.trim_start_matches('+').parse::<usize>().map_err(|e| e.to_string())?;
        Ok(Self((lr, ur)))
    }
}

#[derive(FromArgs)]
#[argh(description = "Commands.")]
pub struct Commands {
    #[argh(subcommand)]
    pub cmds: CommandEnum,
}

#[derive(FromArgs)]
#[argh(subcommand)]
pub enum CommandEnum {
    Scanner(SubCommandScan),
    Convert(SubCommandConvert),
    Diff(SubCommandDiff),
}

#[derive(FromArgs)]
#[argh(subcommand, name = "scan", description = "scanner")]
pub struct SubCommandScan {
    #[argh(option, short = 'f', description = "file path")]
    pub file: PathBuf,
    #[argh(option, short = 't', description = "target address")]
    pub target: Target,
    #[argh(option, default = "7", short = 'd', description = "depth")]
    pub depth: usize,
    #[argh(option, default = "Offset((0, 600))", short = 'o', description = "offset")]
    pub offset: Offset,
    #[argh(option, description = "out file")]
    pub out: Option<PathBuf>,
}

impl SubCommandScan {
    /// name: once output path is not provided, ${name}.scandata will be
    /// generated (pmap, mmap): ...
    /// target: target address you are intrested in
    /// out: output path
    /// depth: pointer search depth. 7 is generally a good choice
    /// offset: (ahead, behind) means, for example, you have target address `p`,
    ///     PtrSX will iterate over P.offset(-ahead) ..= P.offset(behind), for a
    /// pointer points to p
    pub fn perform(
        name: &OsStr,
        (pmap, mmap): (BTreeMap<usize, usize>, Vec<Map>),
        selected_regions: Option<Vec<Map>>,
        target: Target,
        out: Option<PathBuf>,
        depth: usize,
        offset: Offset,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let selected_regions = if let Some(regions) = selected_regions {
            regions
        } else {
            select_module(mmap)?
        };

        let mut spinner = Spinner::start("Start creating pointer maps...");
        let points = selected_regions
            .iter()
            .flat_map(|Map { start, end, path: _ }| pmap.range((Included(start), Included(end))).map(|(&k, _)| k))
            .collect::<Vec<_>>();
        let mut map: BTreeMap<Address, Vec<Address>> = BTreeMap::new();
        for (k, v) in pmap {
            map.entry(v).or_default().push(k);
        }
        spinner.stop("Pointer map is created.");

        let mut spinner = Spinner::start("Start scanning pointer path...");

        let out = match out {
            Some(file) => OpenOptions::new().write(true).append(true).create_new(true).open(file),
            None => OpenOptions::new()
                .write(true)
                .append(true)
                .create_new(true)
                .open(PathBuf::from(name).with_extension("scandata")),
        }?;
        let mut out = BufWriter::with_capacity(MAX_BUF_SIZE, out);

        encode_map_to_writer(selected_regions, &mut out)?;

        PathFindEngine {
            target: target.0,
            depth,
            offset: offset.0,
            out: &mut out,
            startpoints: points,
            engine: PointerSeacher(map),
        }
        .find_pointer_path()?;

        spinner.stop("Pointer path is scanned.");
        Ok(())
    }

    pub fn init(self) -> Result<(), Box<dyn std::error::Error>> {
        let SubCommandScan { file, target, out, depth, offset } = self;
        let name = file.file_stem().ok_or("Get file name error")?;
        let mut spinner = Spinner::start("Start loading cache...");
        let map = load_pointer_map(&file)?;
        spinner.stop("cache loaded.");

        SubCommandScan::perform(name, map, None, target, out, depth, offset)
    }
}

pub struct PathFindEngine<'a, W> {
    target: Address,
    depth: usize,
    offset: (usize, usize),
    out: &'a mut W,
    startpoints: Vec<Address>,
    engine: PointerSeacher,
}

impl<W> PathFindEngine<'_, W>
where
    W: io::Write,
{
    pub fn find_pointer_path(self) -> io::Result<()> {
        let PathFindEngine { target, depth, offset, out, engine, startpoints } = self;
        let size = depth * 2 + 9;
        out.write_all(&size.to_le_bytes())?;
        engine.path_find_helpers(target, out, offset, depth, size, &startpoints)
    }
}

#[derive(FromArgs)]
#[argh(subcommand, name = "conv", description = "convert bin to txt")]
pub struct SubCommandConvert {
    #[argh(option, short = 'f', description = "file path")]
    pub file: PathBuf,
    #[argh(option, description = "out file name")]
    pub out: Option<PathBuf>,
}

impl SubCommandConvert {
    pub fn init(self) -> Result<(), Box<dyn std::error::Error>> {
        let SubCommandConvert { file, out } = self;

        let out: Box<dyn Write> = match out {
            Some(file) => Box::new(
                OpenOptions::new()
                    .write(true)
                    .append(true)
                    .create_new(true)
                    .open(file)?,
            ) as _,
            None => Box::new(io::stdout()) as _,
        };
        let out = BufWriter::with_capacity(MAX_BUF_SIZE, out);

        convert_bin_to_txt(file, out)
    }
}

#[derive(FromArgs)]
#[argh(subcommand, name = "diff", description = "diff")]
pub struct SubCommandDiff {
    #[argh(option, description = "file1 path")]
    pub f1: PathBuf,
    #[argh(option, description = "file2 path")]
    pub f2: PathBuf,
    #[argh(option, description = "out file name")]
    pub out: Option<PathBuf>,
}

impl SubCommandDiff {
    pub fn init(self) -> Result<(), Box<dyn std::error::Error>> {
        let SubCommandDiff { f1, f2, out } = self;

        let h1 = unsafe { String::from_utf8_unchecked(fs::read(f1)?) };
        let h1 = h1.lines().collect::<HashSet<_>>();

        let h2 = unsafe { String::from_utf8_unchecked(fs::read(f2)?) };
        let h2 = h2.lines().collect::<HashSet<_>>();

        let out: Box<dyn Write> = match out {
            Some(file) => Box::new(
                OpenOptions::new()
                    .write(true)
                    .append(true)
                    .create_new(true)
                    .open(file)?,
            ) as _,
            None => Box::new(io::stdout()) as _,
        };
        let mut out = BufWriter::with_capacity(MAX_BUF_SIZE, out);

        Ok(h1.intersection(&h2).try_for_each(|s| writeln!(out, "{s}"))?)
    }
}
