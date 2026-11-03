use std::fmt::Write;

use crate::utils;

pub struct Render;

/// How [`Render::result_list`] introduces its items.
enum ListHeading<'a> {
    /// A `###` section heading.
    Section(&'a str),
    /// A `##` report heading.
    Report(&'a str),
    /// A plain label line, such as `Caveats:`.
    Label(&'a str),
}

mod context;
mod explain;
mod history;
mod impact;
mod map;
mod orientation;

impl Render {
    fn commits(output: &mut String, commits: &[dalil_core::CommitEvidence]) {
        writeln!(output, "#### Evidence commits").expect("writing to a string cannot fail");
        if commits.is_empty() {
            writeln!(output, "No matching commits were found.").expect("writing to a string cannot fail");
        } else {
            for commit in commits {
                let paths =
                    if commit.paths.is_empty() { "no in-scope paths".to_owned() } else { commit.paths.join(", ") };
                writeln!(
                    output,
                    "- `{}` — {} ({}){}",
                    utils::escape_inline_code(&commit.id),
                    utils::sanitize_text(&commit.subject),
                    utils::sanitize_text(&paths),
                    if commit.matched_terms.is_empty() {
                        String::new()
                    } else {
                        format!(" — matched {}", utils::inline_code_list(&commit.matched_terms))
                    }
                )
                .expect("writing to a string cannot fail");
            }
        }
    }

    fn section_heading(output: &mut String, heading: &str) {
        writeln!(output).expect("writing to a string cannot fail");
        writeln!(output, "### {heading}").expect("writing to a string cannot fail");
        writeln!(output).expect("writing to a string cannot fail");
    }

    /// Prints the caveats a result reached under "Caveats:", leaving out
    /// method limitations without a note. `explain` lists those in its
    /// "Method limitations" section.
    fn caveats(output: &mut String, caveats: &[String]) {
        let (kept, _) = utils::result_limitations(caveats.iter().map(String::as_str));
        Render::result_list(output, ListHeading::Label("Caveats:"), &kept, 0);
    }

    /// Prints the caveats a result reached and one line in place of any method
    /// limitations.
    fn result_caveats(output: &mut String, caveats: &[String]) {
        let (kept, omitted) = utils::result_limitations(caveats.iter().map(String::as_str));
        Render::result_list(output, ListHeading::Label("Caveats:"), &kept, omitted);
    }

    /// Prints the method limitations among `limitations` under their own
    /// heading, each once, in the order first seen.
    pub fn method_limitations<'a>(output: &mut String, limitations: impl IntoIterator<Item = &'a str>) {
        let mut method = Vec::new();
        for limitation in limitations {
            if dalil_core::limitations::is_method_limitation(limitation) && !method.contains(&limitation) {
                method.push(limitation);
            }
        }
        if method.is_empty() {
            return;
        }
        writeln!(output).expect("writing to a string cannot fail");
        writeln!(output, "Method limitations:").expect("writing to a string cannot fail");
        for limitation in method {
            writeln!(output, "- {}", utils::sanitize_text(limitation)).expect("writing to a string cannot fail");
        }
    }

    /// Prints uncertainty details under a section heading, leaving out method
    /// limitations behind one line. `extra_omitted` counts method limitations
    /// the caller left out elsewhere in the section's report.
    fn result_uncertainty<'a>(
        output: &mut String, heading: &str, details: impl IntoIterator<Item = &'a str>, extra_omitted: usize,
    ) {
        let (kept, omitted) = utils::result_limitations(details);
        Render::result_list(output, ListHeading::Section(heading), &kept, omitted + extra_omitted);
    }

    /// Prints a report's `## Limitations` section, leaving out method
    /// limitations behind one line.
    pub fn report_limitations<'a>(output: &mut String, limitations: impl IntoIterator<Item = &'a str>) {
        let (kept, omitted) = utils::result_limitations(limitations);
        Render::result_list(output, ListHeading::Report("Limitations"), &kept, omitted);
    }

    /// Prints `heading` over `kept`, with the method limitations note as the
    /// last item when `omitted` is not zero. When `kept` is empty, prints the
    /// note as a plain line without the heading.
    fn result_list(output: &mut String, heading: ListHeading<'_>, kept: &[&str], omitted: usize) {
        if kept.is_empty() {
            if omitted > 0 {
                if matches!(heading, ListHeading::Section(_) | ListHeading::Report(_)) {
                    writeln!(output).expect("writing to a string cannot fail");
                }
                writeln!(output, "{}", utils::METHOD_LIMITATIONS_NOTE).expect("writing to a string cannot fail");
            }
            return;
        }
        match heading {
            ListHeading::Section(heading) => Render::section_heading(output, heading),
            ListHeading::Report(heading) => {
                writeln!(output, "\n## {heading}\n").expect("writing to a string cannot fail")
            }
            ListHeading::Label(label) => writeln!(output, "{label}").expect("writing to a string cannot fail"),
        }
        for detail in kept {
            writeln!(output, "- {}", utils::sanitize_text(detail)).expect("writing to a string cannot fail");
        }
        if omitted > 0 {
            writeln!(output, "- {}", utils::METHOD_LIMITATIONS_NOTE).expect("writing to a string cannot fail");
        }
    }

    /// The caveat of a history observation, or `None` when it is a method
    /// limitation that human output leaves out.
    fn observation_caveat(caveat: &str) -> Option<String> {
        (!dalil_core::limitations::is_method_limitation(caveat)).then(|| utils::sanitize_text(caveat))
    }

    fn format_location(location: &dalil_core::SourceLocation) -> String {
        format!(
            "{}:{}-{}:{}",
            location.start.line, location.start.column, location.end.line, location.end.column
        )
    }
}
