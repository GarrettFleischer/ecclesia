//! Shared `?` binds for both store drivers.

#[derive(Clone, Copy)]
pub enum Bind<'a> {
    Text(&'a str),
    OptText(Option<&'a str>),
    I64(i64),
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
