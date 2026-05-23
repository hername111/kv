// 执行器：ExecutionPlan → ResultSet
use crate::ast::Statement;
use kv_common::types::{Row, Value};
use std::collections::HashMap;

// 假设 StorageEngine 接口由 hhy 实现
pub trait StorageEngine {
    fn insert(&mut self, table: &str, row: Row);
    fn select(&self, table: &str) -> Vec<Row>;
    fn update(&mut self, table: &str, updates: Vec<(String, String)>);
    fn delete(&mut self, table: &str);
}

pub struct Executor<'a> {
    pub storage: &'a mut dyn StorageEngine,
}

impl<'a> Executor<'a> {
    pub fn new(storage: &'a mut dyn StorageEngine) -> Self {
        Self { storage }
    }

    pub fn execute(&mut self, stmt: Statement) -> Vec<Row> {
        match stmt {
            Statement::Select { table, .. } => self.storage.select(&table),
            Statement::Insert { table, row } => {
                self.storage.insert(&table, row);
                vec![]
            },
            Statement::Update { table, updates } => {
                self.storage.update(&table, updates);
                vec![]
            },
            Statement::Delete { table } => {
                self.storage.delete(&table);
                vec![]
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::Statement;

    struct MockStorage {
        pub data: HashMap<String, Vec<Row>>,
    }

    impl MockStorage {
        pub fn new() -> Self {
            Self { data: HashMap::new() }
        }
    }

    impl StorageEngine for MockStorage {
        fn insert(&mut self, table: &str, row: Row) {
            self.data.entry(table.to_string()).or_default().push(row);
        }
        fn select(&self, table: &str) -> Vec<Row> {
            self.data.get(table).cloned().unwrap_or_default()
        }
        fn update(&mut self, _table: &str, _updates: Vec<(String, String)>) {}
        fn delete(&mut self, _table: &str) {}
    }

    #[test]
    fn test_executor_insert_select() {
        let mut storage = MockStorage::new();
        let mut exe = Executor::new(&mut storage);
        let row = Row { values: vec![Value::Int(1)] };
        exe.execute(Statement::Insert { table: "t".to_string(), row: row.clone() });
        let res = exe.execute(Statement::Select { table: "t".to_string(), columns: vec!["*".to_string()] });
        assert_eq!(res[0].values, row.values);
    }
}