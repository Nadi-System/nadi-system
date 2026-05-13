use nadi_plugin::nadi_internal_plugin;

#[nadi_internal_plugin]
mod geom {
    use anyhow;
    use geo_types::{LineString, Point};
    use nadi_plugin::env_func;
    use wkt::{ToWkt, TryFromWkt, Wkt};

    #[env_func]
    fn line(start: &str, end: &str) -> Result<String, String> {
        let start: Point = Point::try_from_wkt_str(start).map_err(|e| e.to_string())?;
        let end: Point = Point::try_from_wkt_str(end).map_err(|e| e.to_string())?;
        let line: LineString = vec![start, end].into();
        Ok(line.wkt_string())
    }
}
