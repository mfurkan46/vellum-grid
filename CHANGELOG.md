# Changelog

All notable changes to Vellum Grid are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2026-10-25

First public release.

### Added

- **Spreadsheet engine:** sparse sheets with 1,048,576 rows and 16,384 columns, a dependency graph that recalculates only affected cells, circular reference detection, automatic and manual calculation, 100 levels of undo and redo.
- **Formulas:** 113 functions with Turkish and English names, named ranges, structured table references, colored references while editing, trace precedents and dependents, error checking, Insert Function dialog, suggestions and argument tips while typing.
- **Editing:** Paste Special, fill series, Flash Fill, AutoComplete, insert and delete cells, rows and columns with reference adjustment, drag to move or copy, multiple selections, Go To and Go To Special, Find and Replace with wildcards, AutoRecover.
- **Formatting:** fonts, colors, fills and patterns, borders, alignment and rotation, number formats with custom codes, Format Cells dialog, Cell Styles, Format Painter, 60 table styles, conditional formatting with data bars, color scales and icon sets.
- **Data tools:** multi-level sort with Turkish alphabetical order, AutoFilter with value, number, date, text and color filters, Advanced Filter, Data Validation, Text to Columns, Remove Duplicates, text import, grouping and outlines, Subtotal, freeze panes.
- **Charts and objects:** column, bar, line, area, pie, doughnut and scatter charts with subtypes and chart elements, sparklines, pictures, shapes and text boxes.
- **Review:** notes, threaded comments, hyperlinks, sheet and workbook protection, spell checking with built-in Turkish and English (US) dictionaries.
- **Files:** `.xlsx` and `.ods` reading and writing, `.xls` reading, CSV in UTF-8 and Windows-1254, File screen with recent files.
- **Printing:** page setup, print titles, headers and footers, page breaks, print preview, Page Break Preview, PDF export with embedded fonts.
- **Interface:** ribbon, custom title bar, context menus, status bar statistics and zoom, Turkish and English interface, light, dark and system themes with a choice of dark or white sheet background, short animations in the ribbon and the File screen.
- **Platforms:** Windows and Linux desktop builds, a Debian package for Pardus, Debian and Ubuntu, Windows install script with file associations, and a WebAssembly build for the browser.
- **Command-line tool:** `vellum-grid-cli` to read, change and export CSV files.

[Unreleased]: https://github.com/mfurkan46/vellum-grid/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/mfurkan46/vellum-grid/releases/tag/v0.1.0
