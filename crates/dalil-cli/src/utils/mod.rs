pub fn escape_inline_code(input: &str) -> String {
    sanitize_text(input).replace('`', "′")
}

pub fn inline_code_list(values: &[String]) -> String {
    values
        .iter()
        .map(|value| format!("`{}`", escape_inline_code(value)))
        .collect::<Vec<_>>()
        .join(", ")
}

pub fn escape_markdown(input: &str) -> String {
    let mut output = String::new();
    for character in sanitize_text(input).chars() {
        if matches!(character, '\\' | '*' | '_' | '[' | ']') {
            output.push('\\');
        }
        output.push(character);
    }
    output
}

pub fn sanitize_text(input: &str) -> String {
    input
        .chars()
        .map(|character| if character.is_control() { '�' } else { character })
        .collect()
}

pub fn token_count(text: &str) -> usize {
    text.chars().count().div_ceil(4).max(1)
}

/// The line human output prints in place of method limitations.
pub const METHOD_LIMITATIONS_NOTE: &str =
    "Method limitations that hold for every run are left out here; `--json` lists them in full.";

/// Splits limitations into the ones a result reached, in order, and the number
/// of method limitations left out.
pub fn result_limitations<'a>(limitations: impl IntoIterator<Item = &'a str>) -> (Vec<&'a str>, usize) {
    let mut omitted = 0;
    let kept = limitations
        .into_iter()
        .filter(|limitation| {
            let method = dalil_core::limitations::is_method_limitation(limitation);
            omitted += usize::from(method);
            !method
        })
        .collect();
    (kept, omitted)
}
