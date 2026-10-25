use std::error::Error;
use std::fs::File;
use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;

use vellum_grid_core::{Cell, CellRange, Document, Workbook, sheet_name_from};
use vellum_grid_io::csv;

const USAGE: &str = "\
usage: vellum-grid-cli <input.csv> [command]...

Loads a CSV file into a sheet and runs the commands in order.

commands:
  get <cell>           print the cell value (formulas are calculated)
  set <cell> <value>   write a value or a formula (=SUM(A1:A3)); '... forces text
  clear <cell>         delete the cell
  export <file|->      write the sheet as CSV (- for stdout)";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: &[String]) -> Result<(), Box<dyn Error>> {
    let Some((input, mut rest)) = args.split_first() else {
        return Err(USAGE.into());
    };
    if input == "-h" || input == "--help" {
        println!("{USAGE}");
        return Ok(());
    }

    let file = File::open(input).map_err(|e| format!("{input}: {e}"))?;
    let name = Path::new(input)
        .file_stem()
        .and_then(|stem| sheet_name_from(&stem.to_string_lossy()))
        .unwrap_or_else(|| "Sheet1".to_owned());
    let sheet = csv::read_sheet(&name, file).map_err(|e| format!("{input}: {e}"))?;
    let mut doc = Document::new(Workbook::from_sheets(vec![sheet])?);
    let mut stdout = io::stdout().lock();

    while let Some((command, tail)) = rest.split_first() {
        rest = match (command.as_str(), tail) {
            ("get", [cell, tail @ ..]) => {
                let content = doc.sheet().get(cell.parse()?).map(|c| c.value.to_string());
                writeln!(stdout, "{}", content.unwrap_or_default())?;
                tail
            }
            ("set", [cell, value, tail @ ..]) => {
                doc.edit([(cell.parse()?, Cell::parse_in(value, doc.locale())?)]);
                tail
            }
            ("clear", [cell, tail @ ..]) => {
                doc.clear(CellRange::single(cell.parse()?));
                tail
            }
            ("export", [path, tail @ ..]) => {
                if path == "-" {
                    csv::write_sheet(doc.sheet(), &mut stdout)?;
                } else {
                    let file = File::create(path).map_err(|e| format!("{path}: {e}"))?;
                    csv::write_sheet(doc.sheet(), file)?;
                }
                tail
            }
            _ => return Err(format!("unexpected argument '{command}'\n\n{USAGE}").into()),
        };
    }
    Ok(())
}
