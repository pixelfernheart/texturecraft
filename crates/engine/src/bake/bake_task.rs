use super::{MergedHigh, StreamedBake, StreamedMap};
use crate::{EngineError, Result};
use std::sync::Arc;
use texturecraft_bake::{BakeOutput, BakePaintMap, BakeRequest, HighPoly, Progress, bake_tiles, tracing::Tracer};
use texturecraft_compose::BakedMaps;
use texturecraft_core::{BakeSettings, BakedMap};
use texturecraft_geom::{Bvh, Mesh};

/// A self-contained bake task that can run on another thread.
pub struct BakeTask {
    pub low_base_color: super::MaterialColors,
    pub ts: usize,
    pub mesh: Arc<Mesh>,
    pub bvh: Arc<Bvh>,
    pub material: usize,
    pub settings: BakeSettings,
    /// The merged high polys, unless the set bakes from the low poly.
    pub high: Option<MergedHigh>,
    pub cage: Option<Arc<Mesh>>,
    pub skew: Option<Arc<BakePaintMap>>,
    pub cage_paint: Option<Arc<BakePaintMap>>,
    /// UV tile grid (columns, rows) of the set; (1, 1) without UV tiles.
    pub tiles: (u32, u32),
    /// Snapshot of tile selection and pixels to retain while the worker runs
    pub excluded_tiles: Vec<(u32, u32)>,
    pub previous_maps: BakedMaps,
    /// The GPU ray tracer, if installed and not turned off (`bake.run {cpu: true}`).
    pub tracer: Option<Arc<dyn Tracer>>,
}

impl BakeTask {
    pub fn maps(&self) -> Vec<BakedMap> {
        self.settings.maps.enabled().collect()
    }

    pub fn run_streaming(&self, progress: &Progress, on_map: &mut dyn FnMut(StreamedMap)) -> Result<StreamedBake> {
        let maps = self.maps();
        let total = maps.len();
        let mut combined: Option<StreamedBake> = None;

        for (index, map) in maps.iter().copied().enumerate() {
            let mut settings = self.settings.clone();
            settings.maps.select([map]);
            let mut output = self.run_with(&settings, progress)?;
            let Some(data) = output.maps.remove(&map) else { continue };
            if combined.is_none() {
                super::preserve_tiles::restore_unchecked(&mut output, &self.previous_maps, &self.excluded_tiles, &maps)?;
            }

            let data = Arc::new(data);
            on_map(StreamedMap { map, index, total, resolution: output.resolution, height: output.height, data: data.clone() });
            match &mut combined {
                Some(combined) => {
                    combined.maps.insert(map, data);
                }
                None => {
                    let mut retained: std::collections::BTreeMap<_, _> = output.maps.into_iter().map(|(map, pixels)| (map, Arc::new(pixels))).collect();
                    retained.insert(map, data);
                    combined = Some(StreamedBake { resolution: output.resolution, height: output.height, maps: retained, tracer: output.tracer });
                }
            }
        }

        match combined {
            Some(output) => Ok(output),
            None => {
                let output = self.run(progress)?;
                Ok(StreamedBake {
                    resolution: output.resolution,
                    height: output.height,
                    maps: output.maps.into_iter().map(|(map, pixels)| (map, Arc::new(pixels))).collect(),
                    tracer: output.tracer,
                })
            }
        }
    }

    pub fn run(&self, progress: &Progress) -> Result<BakeOutput> {
        let mut output = self.run_with(&self.settings, progress)?;
        super::preserve_tiles::restore_unchecked(&mut output, &self.previous_maps, &self.excluded_tiles, &[])?;
        Ok(output)
    }

    fn run_with(&self, settings: &BakeSettings, progress: &Progress) -> Result<BakeOutput> {
        let high =
            self.high.as_ref().map(|source| HighPoly { file_ids: &source.file_ids, mesh: &source.mesh, bvh: &source.bvh, base_color: &source.base_color });
        let request = BakeRequest {
            low_base_color: &self.low_base_color,
            mesh: &self.mesh,
            bvh: &self.bvh,
            material: self.material,
            settings,
            high,
            cage: self.cage.as_deref(),
            skew: self.skew.as_deref(),
            cage_paint: self.cage_paint.as_deref(),
            tiles: self.tiles,
        };
        let mut output = bake_tiles(&request, progress, &self.excluded_tiles, self.tracer.as_deref())
            .map_err(|error| EngineError::Other(format!("bake failed: {error}")))?;
        super::preserve_tiles::restore(&mut output, &self.previous_maps, &self.excluded_tiles, self.tiles)?;
        Ok(output)
    }
}
