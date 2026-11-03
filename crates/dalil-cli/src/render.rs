use std::fmt::Write;

use crate::utils;

pub struct Render;

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

    fn caveats(output: &mut String, caveats: &[String]) {
        if caveats.is_empty() {
            return;
        }
        writeln!(output, "Caveats:").expect("writing to a string cannot fail");
        for caveat in caveats {
            writeln!(output, "- {}", utils::sanitize_text(caveat)).expect("writing to a string cannot fail");
        }
    }

    /// Prints the caveats a result reached and one line in place of any method
    /// limitations. `explain` prints every caveat with [`Render::caveats`].
    fn result_caveats(output: &mut String, caveats: &[String]) {
        let (kept, omitted) = utils::result_limitations(caveats.iter().map(String::as_str));
        if kept.is_empty() && omitted == 0 {
            return;
        }
        writeln!(output, "Caveats:").expect("writing to a string cannot fail");
        for caveat in kept {
            writeln!(output, "- {}", utils::sanitize_text(caveat)).expect("writing to a string cannot fail");
        }
        if omitted > 0 {
            writeln!(output, "- {}", utils::METHOD_LIMITATIONS_NOTE).expect("writing to a string cannot fail");
        }
    }

    /// Prints the method limitations among `limitations` under their own heading.
    pub fn method_limitations(output: &mut String, limitations: &[String]) {
        let method = limitations
            .iter()
            .filter(|limitation| dalil_core::limitations::is_method_limitation(limitation))
            .collect::<Vec<_>>();
        if method.is_empty() {
            return;
        }
        writeln!(output, "Method limitations:").expect("writing to a string cannot fail");
        for limitation in method {
            writeln!(output, "- {}", utils::sanitize_text(limitation)).expect("writing to a string cannot fail");
        }
    }

    /// Prints uncertainty details, leaving out method limitations behind one line.
    fn result_uncertainty<'a>(output: &mut String, details: impl IntoIterator<Item = &'a str>) {
        let (kept, omitted) = utils::result_limitations(details);
        for detail in kept {
            writeln!(output, "- {}", utils::sanitize_text(detail)).expect("writing to a string cannot fail");
        }
        if omitted > 0 {
            writeln!(output, "- {}", utils::METHOD_LIMITATIONS_NOTE).expect("writing to a string cannot fail");
        }
    }

    fn format_location(location: &dalil_core::SourceLocation) -> String {
        format!(
            "{}:{}-{}:{}",
            location.start.line, location.start.column, location.end.line, location.end.column
        )
    }
}
