//! Pure, dependency-free string helpers shared across passes.
//!
//! `normalize_name` and `name_similarity` are ported VERBATIM from FDML's
//! `src/linker/mod.rs`. They were previously duplicated in `fdml-discovery`
//! (and lived inline in the linker); this is the single source of truth so
//! every pass normalizes identifiers identically.

/// Normalize a name for comparison: CamelCase → snake_case, strip underscores.
/// Handles acronyms: SlowAPIMiddleware → slow_api_middleware (not slow_a_p_i_middleware).
pub fn normalize_name(name: &str) -> String {
    let chars: Vec<char> = name.chars().collect();
    let mut result = String::new();
    let len = chars.len();

    for i in 0..len {
        let ch = chars[i];
        if ch.is_uppercase() && i > 0 {
            let prev = chars[i - 1];
            let next = chars.get(i + 1);
            if prev.is_lowercase() || prev.is_ascii_digit() {
                // camelCase boundary: aB → a_b
                result.push('_');
            } else if prev.is_uppercase() {
                // Inside acronym: check if next char is lowercase (end of acronym)
                // e.g. "API" in "SlowAPIMiddleware": at 'I' next='M'(lower) → insert _ before 'I'? No.
                // Actually at 'M' prev='I'(upper), next='i'(lower) → prev.is_upper + next.is_lower → insert _
                if let Some(&n) = next {
                    if n.is_lowercase() {
                        result.push('_');
                    }
                }
            }
        }
        result.push(ch.to_ascii_lowercase());
    }
    result.replace('-', "_").trim_matches('_').to_string()
}

/// Compute similarity score between two normalized names (0.0 - 1.0).
pub fn name_similarity(a: &str, b: &str) -> f64 {
    let na = normalize_name(a);
    let nb = normalize_name(b);

    if na == nb {
        return 1.0;
    }

    // Check if one contains the other
    if na.contains(&nb) || nb.contains(&na) {
        let longer = na.len().max(nb.len()) as f64;
        let shorter = na.len().min(nb.len()) as f64;
        return shorter / longer;
    }

    // Token-based overlap
    let tokens_a: Vec<&str> = na.split('_').filter(|s| !s.is_empty()).collect();
    let tokens_b: Vec<&str> = nb.split('_').filter(|s| !s.is_empty()).collect();

    if tokens_a.is_empty() || tokens_b.is_empty() {
        return 0.0;
    }

    let common = tokens_a.iter().filter(|t| tokens_b.contains(t)).count();
    let total = tokens_a.len().max(tokens_b.len());

    common as f64 / total as f64
}
