use anyhow::Context;

use super::layout::ImageRef;

#[derive(Debug, Clone)]
pub(crate) struct SequenceAnimation {
    pub(crate) frames: Vec<(ImageRef, f32)>,
    duration: f32,
    pub(crate) loops: u32,
}

impl SequenceAnimation {
    pub(crate) fn parse(bytes: &[u8]) -> anyhow::Result<Self> {
        let doc = roxmltree::Document::parse(std::str::from_utf8(bytes)?)?;
        let root = doc.root_element();
        anyhow::ensure!(root.has_tag_name("SequenceImage"), "expected SequenceImage");
        let count: usize = root
            .attribute("FrameCount")
            .context("missing FrameCount")?
            .parse()?;
        let loops = root.attribute("LoopCount").unwrap_or("0").parse()?;
        let mut frames = Vec::new();
        for node in root.children().filter(|n| n.has_tag_name("ImageFrame")) {
            let index: usize = node
                .attribute("Index")
                .context("missing frame Index")?
                .parse()?;
            let duration: f32 = node
                .attribute("Duration")
                .context("missing Duration")?
                .parse()?;
            anyhow::ensure!(
                duration.is_finite() && duration > 0.,
                "invalid frame duration"
            );
            let image = ImageRef {
                set: node
                    .attribute("Imageset")
                    .context("missing Imageset")?
                    .into(),
                image: node.attribute("Image").context("missing Image")?.into(),
            };
            frames.push((index, image, duration));
        }
        frames.sort_by_key(|frame| frame.0);
        anyhow::ensure!(
            count > 0
                && frames.len() == count
                && frames.iter().enumerate().all(|(i, frame)| i == frame.0),
            "invalid sequence indices/count"
        );
        let duration = frames.iter().map(|frame| frame.2).sum::<f32>();
        anyhow::ensure!(duration.is_finite(), "invalid sequence duration");
        Ok(Self {
            frames: frames
                .into_iter()
                .map(|(_, image, duration)| (image, duration))
                .collect(),
            duration,
            loops,
        })
    }

    pub(crate) fn frame(&self, elapsed: f64) -> &ImageRef {
        let total = self.duration as f64;
        if self.loops > 0 && elapsed >= total * self.loops as f64 {
            return &self.frames.last().unwrap().0;
        }
        let mut phase = elapsed % total;
        for (image, duration) in &self.frames {
            if phase < *duration as f64 {
                return image;
            }
            phase -= *duration as f64;
        }
        &self.frames.last().unwrap().0
    }
}
