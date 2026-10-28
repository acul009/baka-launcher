//! Evaluate launcher queries before falling back to application search.

/// Returns fasteval's result as clipboard-ready text, or no result on failure.
pub fn evaluate(query: &str) -> Option<String> {
    fasteval::ez_eval(query, &mut fasteval::EmptyNamespace)
        .ok()
        .map(|value| value.to_string())
}

#[cfg(test)]
mod tests {
    use super::evaluate;

    #[test]
    fn expressions_use_fasteval_operators_functions_and_number_formats() {
        for (query, expected) in [
            ("2 + 3 * 4", "14"),
            ("(2 + 3) * 4", "20"),
            ("9^0.5", "3"),
            ("sin(pi()/2)", "1"),
            ("1k + 25", "1025"),
            ("42", "42"),
            ("1/0", "inf"),
        ] {
            assert_eq!(evaluate(query).as_deref(), Some(expected), "{query}");
        }
    }

    #[test]
    fn app_names_empty_queries_and_invalid_expressions_fall_back_to_search() {
        for query in ["", "  ", "firefox", "VS Code", "2 +", "sin(", "unknown(2)"] {
            assert_eq!(evaluate(query), None, "{query}");
        }
    }
}
