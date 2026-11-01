#![expect(dead_code)]

use console::{Alignment, Style, Term, measure_text_width, pad_str};

pub(crate) struct Column {
    header: &'static str,
    max_width: Option<usize>,
    align: Alignment,
}

impl Column {
    pub(crate) const fn new(header: &'static str) -> Self {
        Self {
            header,
            max_width: None,
            align: Alignment::Left,
        }
    }

    pub(crate) const fn max_width(self, max_width: usize) -> Self {
        Self {
            max_width: Some(max_width),
            ..self
        }
    }

    pub(crate) const fn align(self, align: Alignment) -> Self {
        Self { align, ..self }
    }

    fn width<'a>(&self, cells: impl Iterator<Item = &'a str>) -> usize {
        let content = cells.map(measure_text_width).max().unwrap_or(0);
        let content = self.max_width.map_or(content, |max| content.min(max));
        content.max(measure_text_width(self.header))
    }
}

pub(crate) fn print<const N: usize>(
    term: &Term,
    columns: &[Column; N],
    rows: &[[String; N]],
) -> anyhow::Result<()> {
    let mut lines = render(columns, rows).into_iter();
    if let Some(header) = lines.next() {
        term.write_line(&Style::new().bold().apply_to(header).to_string())?;
    }
    for line in lines {
        term.write_line(&line)?;
    }
    Ok(())
}

fn render<const N: usize>(columns: &[Column; N], rows: &[[String; N]]) -> Vec<String> {
    let widths: [usize; N] =
        std::array::from_fn(|i| columns[i].width(rows.iter().map(|row| row[i].as_str())));
    let header = columns.each_ref().map(|column| column.header);

    std::iter::once(header)
        .chain(rows.iter().map(|row| row.each_ref().map(String::as_str)))
        .map(|cells| render_line(columns, &widths, cells))
        .collect()
}

fn render_line<const N: usize>(
    columns: &[Column; N],
    widths: &[usize; N],
    cells: [&str; N],
) -> String {
    let cells: Vec<_> = (0..N)
        .map(|i| pad_str(cells[i], widths[i], columns[i].align, Some("…")))
        .collect();
    cells.join("  ").trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row<const N: usize>(cells: [&str; N]) -> [String; N] {
        cells.map(String::from)
    }

    #[test]
    fn fits_columns_to_header_and_content() {
        let columns = [Column::new("ID"), Column::new("NAME")];
        let rows = [row(["1", "alpha"]), row(["200", "b"])];

        assert_eq!(
            render(&columns, &rows),
            ["ID   NAME", "1    alpha", "200  b"]
        );
    }

    #[test]
    fn prints_only_header_without_rows() {
        let columns = [Column::new("ID"), Column::new("NAME")];

        assert_eq!(render(&columns, &[]), ["ID  NAME"]);
    }

    #[test]
    fn truncates_cells_over_max_width_with_ellipsis() {
        let columns = [Column::new("NAME").max_width(5), Column::new("KIND")];
        let rows = [row(["abcdefgh", "jira"]), row(["abc", "jira"])];

        assert_eq!(
            render(&columns, &rows),
            ["NAME   KIND", "abcd…  jira", "abc    jira"]
        );
    }

    #[test]
    fn keeps_header_wider_than_max_width() {
        let columns = [Column::new("SUMMARY").max_width(3), Column::new("ID")];
        let rows = [row(["abcdefghij", "1"])];

        assert_eq!(render(&columns, &rows), ["SUMMARY  ID", "abcdef…  1"]);
    }

    #[test]
    fn measures_and_truncates_non_ascii_by_character() {
        let columns = [Column::new("NAME").max_width(4), Column::new("ID")];
        let rows = [row(["čžš", "1"]), row(["ščćžđ", "2"])];

        assert_eq!(render(&columns, &rows), ["NAME  ID", "čžš   1", "ščć…  2"]);
    }

    #[test]
    fn aligns_cells_per_column() {
        let columns = [
            Column::new("ID").align(Alignment::Right),
            Column::new("ACTIVE").align(Alignment::Center),
            Column::new("NAME"),
        ];
        let rows = [row(["7", "*", "a"]), row(["12", "", "b"])];

        assert_eq!(
            render(&columns, &rows),
            ["ID  ACTIVE  NAME", " 7    *     a", "12          b"]
        );
    }

    #[test]
    fn does_not_pad_last_column() {
        let columns = [Column::new("ID"), Column::new("NAME")];
        let rows = [row(["1", "a"]), row(["2", ""])];

        assert_eq!(render(&columns, &rows), ["ID  NAME", "1   a", "2"]);
    }
}
