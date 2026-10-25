pub mod book;
pub mod csv;
pub mod encoding;
pub mod html;
pub mod ods;
mod openformula;
pub mod pdf;
pub mod print;
pub mod tsv;
mod xlsx_drawing;
mod xlsx_extras;
mod xlsx_format;
mod xlsx_post;
mod xlsx_review;

use std::fmt;

use vellum_grid_core::{AddressError, WorkbookError};

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    Csv(::csv::Error),
    Address(AddressError),
    Read(calamine::Error),
    Write(rust_xlsxwriter::XlsxError),
    Workbook(WorkbookError),
    Zip(zip::result::ZipError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => e.fmt(f),
            Self::Csv(e) => e.fmt(f),
            Self::Address(e) => e.fmt(f),
            Self::Read(e) => e.fmt(f),
            Self::Write(e) => e.fmt(f),
            Self::Workbook(e) => e.fmt(f),
            Self::Zip(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Csv(e) => Some(e),
            Self::Address(e) => Some(e),
            Self::Read(e) => Some(e),
            Self::Write(e) => Some(e),
            Self::Workbook(e) => Some(e),
            Self::Zip(e) => Some(e),
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<::csv::Error> for Error {
    fn from(e: ::csv::Error) -> Self {
        Self::Csv(e)
    }
}

impl From<AddressError> for Error {
    fn from(e: AddressError) -> Self {
        Self::Address(e)
    }
}

impl From<calamine::Error> for Error {
    fn from(e: calamine::Error) -> Self {
        Self::Read(e)
    }
}

impl From<rust_xlsxwriter::XlsxError> for Error {
    fn from(e: rust_xlsxwriter::XlsxError) -> Self {
        Self::Write(e)
    }
}

impl From<WorkbookError> for Error {
    fn from(e: WorkbookError) -> Self {
        Self::Workbook(e)
    }
}

impl From<zip::result::ZipError> for Error {
    fn from(e: zip::result::ZipError) -> Self {
        Self::Zip(e)
    }
}
