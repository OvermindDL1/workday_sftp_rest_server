use crate::configuration::BASE_CSV_PATH;
use anyhow::Context as _;
use chrono::{DateTime, Utc};
use glob::glob;
use std::path::PathBuf;
use tracing::*;

#[instrument(level = "debug")]
pub fn get_last_matching_file_delete_rest(globsb: &str, keep: usize) -> anyhow::Result<PathBuf> {
    let globs = format!("{BASE_CSV_PATH}/{globsb}");
    let matches = glob(&globs)?;
    let mut paths: Vec<PathBuf> = matches.collect::<Result<_, _>>()?;
    if paths.is_empty() {
        anyhow::bail!("no matching files for {}", globsb);
    }
    paths.sort();
    if !paths.last().expect("already checked").is_file() {
        anyhow::bail!(
            "last matching path is not a file: {:?}",
            paths.last().expect("already checked")
        );
    }
    if paths.len() > keep {
        info_span!("Too many files matching `{globsb}`, deleting old:").in_scope(
            || -> anyhow::Result<()> {
                for to_del in &paths[0..(paths.len() - keep)] {
                    info!("\t{to_del:?}");
                    std::fs::remove_file(&to_del)?;
                }
                Ok(())
            },
        )?;
    }
    if keep == 0 {
        anyhow::bail!("0 files to keep so cannot return a path");
    }
    debug!(matching_paths = ?paths);
    Ok(paths.pop().expect("already checked"))
}

#[instrument(level = "debug")]
pub fn get_last_matching_file_delete_rest_as_csv<T: for<'de> serde::Deserialize<'de>>(
    globsb: &str,
    keep: usize,
) -> anyhow::Result<Vec<T>> {
    let file_path = get_last_matching_file_delete_rest(globsb, keep)?;
    debug!(?file_path);
    let mut rdr = csv::Reader::from_path(file_path).context("loading csv file")?;
    let data: Vec<T> = rdr
        .deserialize()
        .collect::<Result<_, _>>()
        .context("parsing csv file")?;
    Ok(data)
}

pub fn get_current_datetime() -> DateTime<Utc> {
    Utc::now()
}

#[instrument(level = "debug", skip(data))]
pub fn write_csv_with_date_to<T: serde::Serialize>(
    base_path: &str,
    data: &[T],
    keep: usize,
) -> anyhow::Result<()> {
    let _ignore = get_last_matching_file_delete_rest(&format!("{base_path}_*.csv"), keep);
    let file_path = PathBuf::from(format!(
        "{BASE_CSV_PATH}/{base_path}_{}.csv",
        get_current_datetime().format("%Y-%m-%d-%H-%M-%S-%f")
    ));
    let mut wtr = csv::Writer::from_path(&file_path).context("saving csv file")?;
    for row in data {
        wtr.serialize(row)?;
    }
    drop(wtr);
    info!(path = ?file_path);
    Ok(())
}
