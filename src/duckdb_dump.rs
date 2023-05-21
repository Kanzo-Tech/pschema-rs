use crate::dtype::DataType;
use duckdb::arrow::array::{Array, Int32Array, UInt64Array};
use duckdb::Connection;
use polars::prelude::*;
use pregel_rs::pregel::Column;
use std::path::Path;
use strum::IntoEnumIterator;

pub struct DumpUtils;

/// The `impl DumpUtils` block defines a Rust module that contains `edges_from_duckdb`.
impl DumpUtils {
    /// This function retrieves data from a DuckDB database and returns it as a
    /// DataFrame.
    ///
    /// Arguments:
    ///
    /// * `path`: The path to the DuckDB database file.
    ///
    /// Returns:
    ///
    /// This function returns a `Result<DataFrame, String>`, where the `DataFrame`
    /// is the result of querying and processing data from a DuckDB database, and
    /// the `String` is an error message in case any error occurs during the
    /// execution of the function.
    pub fn edges_from_duckdb(path: &str) -> Result<DataFrame, String> {
        let stmt = DataType::iter()
            .map(|dtype| {
                format!(
                    "SELECT src_id, property_id, dst_id, {:} FROM {:}",
                    u32::from(&dtype),
                    dtype.as_ref()
                )
            })
            .collect::<Vec<String>>()
            .join(" UNION ALL ");

        let connection: Connection = match Path::new(path).try_exists() {
            Ok(true) => match Connection::open(Path::new(path)) {
                Ok(connection) => connection,
                Err(_) => return Err(String::from("Cannot connect to the database")),
            },
            _ => return Err(String::from("Make sure you provide an existing path")),
        };

        let mut statement = match connection.prepare(stmt.as_ref()) {
            Ok(statement) => statement,
            Err(error) => return Err(format!("Cannot prepare the provided statement {}", error)),
        };

        let mut batches = match statement.query_arrow([]) {
            Ok(arrow) => arrow,
            Err(_) => return Err(String::from("Error executing the Arrow query")),
        };

        let mut edges = DataFrame::default();

        while let Some(batch) = batches.next() {
            let srcs = Series::from_vec(
                Column::Src.as_ref(),
                // because we know that the first column is the src_id
                batch
                    .column(0)
                    .as_any()
                    .downcast_ref::<UInt64Array>()
                    .unwrap()
                    .values()
                    .to_vec(),
            );
            let properties = Series::from_vec(
                Column::Custom("property_id").as_ref(),
                // because we know that the second column is the property_id
                batch
                    .column(1)
                    .as_any()
                    .downcast_ref::<UInt64Array>()
                    .unwrap()
                    .values()
                    .to_vec(),
            );
            let dsts = Series::from_vec(
                Column::Dst.as_ref(),
                // because we know that the third column is the dst_id
                batch
                    .column(2)
                    .as_any()
                    .downcast_ref::<UInt64Array>()
                    .unwrap()
                    .values()
                    .to_vec(),
            );
            let dtypes = Series::from_vec(
                Column::Custom("dtype").as_ref(),
                // because we know that the fourth column is the dtype
                batch
                    .column(3)
                    .as_any()
                    .downcast_ref::<Int32Array>()
                    .unwrap()
                    .values()
                    .to_vec(),
            );

            let tmp_dataframe = match DataFrame::new(vec![srcs, properties, dsts, dtypes]) {
                Ok(tmp_dataframe) => tmp_dataframe,
                Err(error) => return Err(format!("Error creating the DataFrame: {}", error)),
            };

            edges = match edges.vstack(&tmp_dataframe) {
                Ok(dataframe) => dataframe,
                Err(_) => return Err(String::from("Error vertically stacking the DataFrames")),
            };
        }

        Ok(edges)
    }
}
