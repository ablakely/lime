use std::{collections::HashMap, io::Write};

use anyhow::{Result, anyhow};

use crate::{
    common::car_uri_components_to_human_readable_file_name,
    uri_path::{
        AbsoluteUriPath, CarUriComponents, FullUriPath, RelativeUriPath, ServerUriPath,
        UriComponent, UriPath, absolute_to_relative, join_absolute_relative_uri_path,
    },
};

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct RelativeOriginalUri(pub FullUriPath);
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct AbsoluteOriginalUri(pub ServerUriPath);
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct AbsoluteAdjustedUri(pub AbsoluteUriPath);
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct RelativeAdjustedUri(pub RelativeUriPath);

pub trait ZipCore: Sized {
    type AdjustCtx;

    type WriteCtx;

    fn adjust_uri(
        &self,
        absolute_original_uri: &AbsoluteOriginalUri,
        ctx: Self::AdjustCtx,
    ) -> Result<(AbsoluteAdjustedUri, Option<Self::WriteCtx>)>;
    /// returns bytes and whether to compress
    fn write(
        &self,
        scoped_zipper: &mut ScopedZipper<Self>,
        ctx: Self::WriteCtx,
    ) -> Result<(Vec<u8>, bool)>;
}

pub struct Zipper<ZC: ZipCore, W: Write> {
    zip_writer: zip::ZipWriter<zip::write::StreamWriter<W>>,
    core: ZC,

    adjustments: HashMap<AbsoluteOriginalUri, AbsoluteAdjustedUri>,
    base_folder_name: String,
    written_page_count: u64,
}

struct WriteQueueItem<ZC: ZipCore> {
    absolute_original_uri: AbsoluteOriginalUri,

    absolute_adjusted_uri: AbsoluteAdjustedUri,
    ctx: ZC::WriteCtx,
}

pub struct ScopedZipper<'a, ZC: ZipCore> {
    adjustments: &'a mut HashMap<AbsoluteOriginalUri, AbsoluteAdjustedUri>,
    queue: &'a mut Vec<WriteQueueItem<ZC>>,

    core: &'a ZC,
    absolute_original_uri: AbsoluteOriginalUri,
    absolute_adjusted_uri: AbsoluteAdjustedUri,
}

impl<'a, ZC: ZipCore> ScopedZipper<'a, ZC> {
    pub fn rou_to_aou(&self, relative_original_uri: RelativeOriginalUri) -> AbsoluteOriginalUri {
        AbsoluteOriginalUri(join_absolute_relative_uri_path(
            &self.absolute_original_uri.0,
            &relative_original_uri.0,
        ))
    }

    pub fn recurse(
        &mut self,
        absolute_original_uri: AbsoluteOriginalUri,
        ctx: ZC::AdjustCtx,
    ) -> RelativeAdjustedUri {
        match self.adjustments.get(&absolute_original_uri) {
            Some(absolute_adjusted_uri) => RelativeAdjustedUri(absolute_to_relative(
                &self.absolute_adjusted_uri.0,
                &absolute_adjusted_uri.0,
            )),
            None => {
                let (absolute_adjusted_uri, write_ctx) =
                    match self.core.adjust_uri(&absolute_original_uri, ctx) {
                        Ok(o) => o,
                        Err(e) => {
                            log::error!("Adjust uri error on aou {absolute_original_uri:?}: {e}");
                            return RelativeAdjustedUri(RelativeUriPath {
                                dirs: vec![],
                                file: Some(
                                    UriComponent::from_decoded_str("adjust_uri_error.html")
                                        .unwrap(),
                                ),
                                fragment: None,
                            });
                        }
                    };
                self.adjustments
                    .insert(absolute_original_uri.clone(), absolute_adjusted_uri.clone());
                if let Some(write_ctx) = write_ctx {
                    self.queue.push(WriteQueueItem {
                        absolute_original_uri,
                        absolute_adjusted_uri: absolute_adjusted_uri.clone(),
                        ctx: write_ctx,
                    });
                }

                RelativeAdjustedUri(absolute_to_relative(
                    &self.absolute_adjusted_uri.0,
                    &absolute_adjusted_uri.0,
                ))
            }
        }
    }
}

impl<ZC: ZipCore, W: Write> Zipper<ZC, W> {
    pub fn recurse(
        &mut self,
        absolute_original_uri: AbsoluteOriginalUri,
        ctx: ZC::AdjustCtx,
    ) -> Result<()> {
        let (absolute_adjusted_uri, write_ctx) =
            self.core.adjust_uri(&absolute_original_uri, ctx)?;
        if let Some(write_ctx) = write_ctx {
            let mut write_queue = vec![WriteQueueItem {
                absolute_original_uri,
                absolute_adjusted_uri,
                ctx: write_ctx,
            }];
            'write_dequeue_loop: while let Some(write_queue_item) = write_queue.pop() {
                let mut scoped_zipper = ScopedZipper {
                    adjustments: &mut self.adjustments,
                    queue: &mut write_queue,
                    core: &self.core,
                    absolute_original_uri: write_queue_item.absolute_original_uri,
                    absolute_adjusted_uri: write_queue_item.absolute_adjusted_uri.clone(),
                };
                let (bytes, compress) =
                    match self.core.write(&mut scoped_zipper, write_queue_item.ctx) {
                        Ok(o) => o,
                        Err(e) => {
                            log::error!(
                                "Zip core write error for AOU {:?}: {e}",
                                &scoped_zipper.absolute_original_uri
                            );
                            continue 'write_dequeue_loop;
                        }
                    };

                self.write(&write_queue_item.absolute_adjusted_uri, &bytes, compress)?;
            }
        }
        Ok(())
    }

    pub fn new(car_uri_components: &CarUriComponents, core: ZC, writer: W) -> Self {
        let mut zip_writer = zip::ZipWriter::new_stream(writer);
        zip_writer.set_comment("Provided by LEMON Manuals (Liberated Excellent Manuals Online) https://lemon-manuals.la https://lemon-manuals.org.ua https://lemon-manuals.gy");
        Self {
            core,
            zip_writer,
            adjustments: HashMap::new(),
            written_page_count: 0,
            base_folder_name: car_uri_components_to_human_readable_file_name(car_uri_components),
        }
    }

    pub fn add_static_files(&mut self, files: &[(ServerUriPath, impl AsRef<[u8]>)]) -> Result<()> {
        for (uri_path, contents) in files {
            let aou = AbsoluteOriginalUri(uri_path.clone());
            let aau = AbsoluteAdjustedUri(uri_path.clone().into());
            self.write(&aau, contents.as_ref(), true)?;
            self.explicit_adjustment(aou, aau)?;
        }
        Ok(())
    }

    pub fn explicit_adjustment(
        &mut self,
        aou: AbsoluteOriginalUri,
        aau: AbsoluteAdjustedUri,
    ) -> Result<()> {
        match self.adjustments.insert(aou, aau) {
            Some(_) => Err(anyhow!("Explicitly added a duplicate adjustment")),
            None => Ok(()),
        }
    }

    pub fn write(
        &mut self,
        absolute_adjusted_uri: &AbsoluteAdjustedUri,
        contents: &[u8],
        compress: bool,
    ) -> Result<()> {
        self.written_page_count += 1;
        if self.written_page_count.is_multiple_of(100) {
            log::debug!("Written {} pages", self.written_page_count);
        }
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(if compress {
                zip::CompressionMethod::DEFLATE
            } else {
                zip::CompressionMethod::STORE
            })
            .last_modified_time(zip::DateTime::from_date_and_time(2026, 3, 1, 0, 0, 0).unwrap());

        let string_file_path =
            self.base_folder_name.clone() + &String::from(absolute_adjusted_uri.0.stringify());
        self.zip_writer.start_file(string_file_path, options)?;
        self.zip_writer.write_all(contents)?;
        Ok(())
    }

    pub fn finish(self) -> Result<W> {
        let mut result = self.zip_writer.finish()?.into_inner();
        result.flush()?;
        Ok(result)
    }
}
