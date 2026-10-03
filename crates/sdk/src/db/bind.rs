//! Shared `?` binds for both store drivers.

#[derive(Clone, Copy)]
pub enum Bind<'a> {
    Text(&'a str),
    OptText(Option<&'a str>),
    I64(i64),
    F64(f64),
}

pub fn placeholders(count: usize) -> String {
    let mut sql = String::new();
    for index in 0..count {
        if index > 0 {
            sql.push(',');
        }
        sql.push('?');
    }
    sql
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn config() -> ProptestConfig {
        ProptestConfig {
            cases: 64,
            max_shrink_iters: 256,
            ..ProptestConfig::default()
        }
    }

    proptest! {
        #![proptest_config(config())]

        #[test]
        fn us_prop_sql_01_placeholders_are_question_marks(count in 0usize..24) {
            let sql = placeholders(count);
            assert_eq!(sql.chars().filter(|ch| *ch == '?').count(), count);
            assert_eq!(sql.chars().filter(|ch| *ch == ',').count(), count.saturating_sub(1));
            assert!(!sql.contains(' '));
            if count == 0 {
                assert!(sql.is_empty());
            } else {
                assert!(sql.starts_with('?'));
                assert!(sql.ends_with('?'));
            }
        }
    }
}
