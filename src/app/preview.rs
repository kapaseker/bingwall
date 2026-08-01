//! Owns Wallpaper Preview scheduling and GPU residency policy.

use std::{
    collections::{HashMap, HashSet, VecDeque},
    path::PathBuf,
};

use iced::widget::image as iced_image;

use crate::feed::WallpaperEntry;

const MAX_ACTIVE_PREVIEWS: usize = 2;
const DEFAULT_GPU_PRELOAD_LIMIT: usize = 4;

/// Owns Wallpaper Preview scheduling and residency policy.
#[derive(Debug)]
pub(crate) struct PreviewResidency {
    preview_paths: HashMap<String, PathBuf>,
    allocations: HashMap<String, iced_image::Allocation>,
    queued_acquisitions: VecDeque<WallpaperEntry>,
    active_acquisitions: HashSet<String>,
    failed_acquisitions: HashSet<String>,
    allocating: HashSet<String>,
    failed_allocations: HashSet<String>,
    invalidated_previews: HashSet<String>,
    retried_selected_allocations: HashSet<String>,
    gpu_preload_limit: usize,
}

/// Describes external work requested by the Wallpaper Preview module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PreviewCommand {
    Acquire(WallpaperEntry),
    Allocate {
        image_url: String,
        path: PathBuf,
    },
    RemoveInvalid {
        image_url: String,
        path: Option<PathBuf>,
    },
}

/// Describes completed external work returned to the Wallpaper Preview module.
#[derive(Debug, Clone)]
pub(crate) enum PreviewEvent {
    Acquired {
        image_url: String,
        result: Result<PathBuf, String>,
    },
    Allocated {
        image_url: String,
        allocation: iced_image::Allocation,
    },
    AllocationFailed {
        image_url: String,
        error: iced_image::Error,
    },
    Invalidated {
        image_url: String,
        result: Result<(), String>,
    },
}

/// Identifies a selected Wallpaper Preview failure without choosing UI copy.
#[derive(Debug, Clone)]
pub(crate) enum PreviewFailure {
    Acquisition(String),
    Allocation(iced_image::Error),
    Invalidation(String),
}

/// Contains commands and any selected-preview failure produced by an event.
#[derive(Debug, Default)]
pub(crate) struct PreviewUpdate {
    pub(crate) commands: Vec<PreviewCommand>,
    pub(crate) selected_failure: Option<PreviewFailure>,
}

impl PreviewResidency {
    /// Creates empty preview residency state with the default GPU preload limit.
    pub(crate) fn new() -> Self {
        Self {
            gpu_preload_limit: DEFAULT_GPU_PRELOAD_LIMIT,
            preview_paths: HashMap::new(),
            allocations: HashMap::new(),
            queued_acquisitions: VecDeque::new(),
            active_acquisitions: HashSet::new(),
            failed_acquisitions: HashSet::new(),
            allocating: HashSet::new(),
            failed_allocations: HashSet::new(),
            invalidated_previews: HashSet::new(),
            retried_selected_allocations: HashSet::new(),
        }
    }

    /// Returns the resident GPU handle for a Wallpaper Preview when available.
    pub(crate) fn handle_for(&self, image_url: &str) -> Option<iced_image::Handle> {
        self.allocations
            .get(image_url)
            .map(|allocation| allocation.handle().clone())
    }

    /// Allows a Wallpaper Preview to retry after the user selects it again.
    pub(crate) fn retry(&mut self, image_url: &str) {
        self.failed_acquisitions.remove(image_url);
        self.failed_allocations.remove(image_url);
    }

    /// Allows acquisition failures to retry after an explicit feed refresh.
    pub(crate) fn retry_acquisitions(&mut self) {
        self.failed_acquisitions.clear();
    }

    /// Drops queued and failed work that no longer belongs to the browsed Feed.
    pub(crate) fn source_changed(&mut self) {
        self.queued_acquisitions.clear();
        self.failed_acquisitions.clear();
        self.failed_allocations.clear();
        self.retried_selected_allocations.clear();
    }

    /// Reconciles desired Wallpaper Previews and returns work in priority order.
    pub(crate) fn reconcile(&mut self, desired_entries: &[WallpaperEntry]) -> Vec<PreviewCommand> {
        let gpu_urls = desired_entries
            .iter()
            .take(self.gpu_preload_limit)
            .map(|entry| entry.image_url.as_str())
            .collect::<HashSet<_>>();
        self.allocations
            .retain(|image_url, _| gpu_urls.contains(image_url.as_str()));

        for entry in desired_entries {
            let image_url = &entry.image_url;
            if !self.preview_paths.contains_key(image_url)
                && !self.active_acquisitions.contains(image_url)
                && !self.failed_acquisitions.contains(image_url)
                && !self
                    .queued_acquisitions
                    .iter()
                    .any(|queued| queued.image_url == *image_url)
            {
                self.queued_acquisitions.push_back(entry.clone());
            }
        }

        let priority = desired_entries
            .iter()
            .enumerate()
            .map(|(rank, entry)| (entry.image_url.as_str(), rank))
            .collect::<HashMap<_, _>>();
        self.queued_acquisitions
            .make_contiguous()
            .sort_by_key(|entry| {
                priority
                    .get(entry.image_url.as_str())
                    .copied()
                    .unwrap_or(usize::MAX)
            });

        let mut commands = Vec::new();
        while self.active_acquisitions.len() < MAX_ACTIVE_PREVIEWS {
            let Some(entry) = self.queued_acquisitions.pop_front() else {
                break;
            };
            self.active_acquisitions.insert(entry.image_url.clone());
            commands.push(PreviewCommand::Acquire(entry));
        }

        for entry in desired_entries.iter().take(self.gpu_preload_limit) {
            let image_url = &entry.image_url;
            if self.allocations.contains_key(image_url)
                || self.failed_allocations.contains(image_url)
                || !self.allocating.insert(image_url.clone())
            {
                continue;
            }
            let Some(path) = self.preview_paths.get(image_url).cloned() else {
                self.allocating.remove(image_url);
                continue;
            };
            commands.push(PreviewCommand::Allocate {
                image_url: image_url.clone(),
                path,
            });
        }
        commands
    }

    /// Applies completed preview work and requests the next required commands.
    pub(crate) fn handle(
        &mut self,
        event: PreviewEvent,
        desired_entries: &[WallpaperEntry],
    ) -> PreviewUpdate {
        let mut selected_failure = None;
        match event {
            PreviewEvent::Acquired { image_url, result } => {
                self.active_acquisitions.remove(&image_url);
                match result {
                    Ok(path) => {
                        self.failed_acquisitions.remove(&image_url);
                        self.failed_allocations.remove(&image_url);
                        self.preview_paths.insert(image_url, path);
                    }
                    Err(error) => {
                        self.failed_acquisitions.insert(image_url.clone());
                        if is_selected(desired_entries, &image_url) {
                            selected_failure = Some(PreviewFailure::Acquisition(error));
                        }
                    }
                }
            }
            PreviewEvent::Allocated {
                image_url,
                allocation,
            } => {
                self.allocating.remove(&image_url);
                if desired_entries
                    .iter()
                    .any(|entry| entry.image_url == image_url)
                {
                    self.allocations.insert(image_url, allocation);
                }
            }
            PreviewEvent::AllocationFailed { image_url, error } => {
                self.allocating.remove(&image_url);
                let is_selected = is_selected(desired_entries, &image_url);
                if matches!(error, iced_image::Error::OutOfMemory) {
                    self.gpu_preload_limit = self.gpu_preload_limit.saturating_sub(1).max(1);
                    if !is_selected || !self.retried_selected_allocations.insert(image_url.clone())
                    {
                        self.failed_allocations.insert(image_url);
                        if is_selected {
                            selected_failure = Some(PreviewFailure::Allocation(error));
                        }
                    }
                } else if matches!(
                    error,
                    iced_image::Error::Invalid(_)
                        | iced_image::Error::Inaccessible(_)
                        | iced_image::Error::Empty
                ) {
                    if self.invalidated_previews.insert(image_url.clone()) {
                        let path = self.preview_paths.remove(&image_url);
                        return PreviewUpdate {
                            commands: vec![PreviewCommand::RemoveInvalid { image_url, path }],
                            selected_failure: None,
                        };
                    }
                    self.failed_allocations.insert(image_url);
                    if is_selected {
                        selected_failure = Some(PreviewFailure::Allocation(error));
                    }
                } else {
                    self.failed_allocations.insert(image_url);
                    if is_selected {
                        selected_failure = Some(PreviewFailure::Allocation(error));
                    }
                }
            }
            PreviewEvent::Invalidated { image_url, result } => {
                self.failed_allocations.remove(&image_url);
                match result {
                    Ok(()) => {
                        self.failed_acquisitions.remove(&image_url);
                    }
                    Err(error) => {
                        self.failed_acquisitions.insert(image_url.clone());
                        if is_selected(desired_entries, &image_url) {
                            selected_failure = Some(PreviewFailure::Invalidation(error));
                        }
                    }
                }
            }
        }
        PreviewUpdate {
            commands: self.reconcile(desired_entries),
            selected_failure,
        }
    }
}

/// Reports whether an image URL identifies the selected Wallpaper Entry.
fn is_selected(desired_entries: &[WallpaperEntry], image_url: &str) -> bool {
    desired_entries
        .first()
        .is_some_and(|entry| entry.image_url == image_url)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a Wallpaper Entry with a stable test identity.
    fn entry(index: usize) -> WallpaperEntry {
        WallpaperEntry {
            date: format!("2026-01-{index:02}"),
            description: format!("Wallpaper {index}"),
            image_url: format!("https://cn.bing.com/{index}.jpg"),
        }
    }

    #[test]
    /// Verifies acquisition starts for only the two highest-priority previews.
    fn starts_two_acquisitions_in_priority_order() {
        let desired = (0..4).map(entry).collect::<Vec<_>>();
        let mut residency = PreviewResidency::new();

        let commands = residency.reconcile(&desired);

        assert_eq!(
            commands,
            vec![
                PreviewCommand::Acquire(desired[0].clone()),
                PreviewCommand::Acquire(desired[1].clone()),
            ]
        );
    }

    #[test]
    /// Verifies queued work is reprioritized without cancelling active acquisitions.
    fn reprioritizes_queued_work_after_an_acquisition_finishes() {
        let entries = (0..4).map(entry).collect::<Vec<_>>();
        let mut residency = PreviewResidency::new();
        let _ = residency.reconcile(&entries);
        let desired = vec![
            entries[1].clone(),
            entries[0].clone(),
            entries[3].clone(),
            entries[2].clone(),
        ];
        let _ = residency.reconcile(&desired);

        let update = residency.handle(
            PreviewEvent::Acquired {
                image_url: entries[0].image_url.clone(),
                result: Ok(PathBuf::from("preview-0.jpg")),
            },
            &desired,
        );

        assert_eq!(
            update.commands,
            vec![
                PreviewCommand::Acquire(entries[3].clone()),
                PreviewCommand::Allocate {
                    image_url: entries[0].image_url.clone(),
                    path: PathBuf::from("preview-0.jpg"),
                },
            ]
        );
    }

    #[test]
    /// Verifies switching sources drops old queued work without cancelling active work.
    fn source_change_discards_queued_preview_acquisitions() {
        let old = (0..4).map(entry).collect::<Vec<_>>();
        let new = vec![WallpaperEntry {
            date: "2026-08-01".into(),
            description: "Spotlight".into(),
            image_url: "https://windows10spotlight.com/new.jpg".into(),
        }];
        let mut residency = PreviewResidency::new();
        let _ = residency.reconcile(&old);

        residency.source_changed();
        let update = residency.handle(
            PreviewEvent::Acquired {
                image_url: old[0].image_url.clone(),
                result: Ok(PathBuf::from("old-preview.jpg")),
            },
            &new,
        );

        assert_eq!(
            update.commands,
            vec![PreviewCommand::Acquire(new[0].clone())]
        );
    }

    #[test]
    /// Verifies off-window acquisition results remain reusable without immediate allocation.
    fn keeps_completed_off_window_preview_on_disk() {
        let old = entry(0);
        let current = entry(1);
        let mut residency = PreviewResidency::new();
        let _ = residency.reconcile(&[old.clone(), current.clone()]);

        let completed = residency.handle(
            PreviewEvent::Acquired {
                image_url: old.image_url.clone(),
                result: Ok(PathBuf::from("old-preview.jpg")),
            },
            std::slice::from_ref(&current),
        );
        assert!(!completed.commands.iter().any(|command| matches!(
            command,
            PreviewCommand::Allocate { image_url, .. } if image_url == &old.image_url
        )));

        let reused = residency.reconcile(std::slice::from_ref(&old));
        assert!(reused.iter().any(|command| matches!(
            command,
            PreviewCommand::Allocate { image_url, path }
                if image_url == &old.image_url && path == &PathBuf::from("old-preview.jpg")
        )));
        assert!(!reused.iter().any(|command| matches!(
            command,
            PreviewCommand::Acquire(entry) if entry.image_url == old.image_url
        )));
    }

    #[test]
    /// Verifies GPU exhaustion retries the selected preview once before reporting failure.
    fn retries_selected_allocation_once_after_gpu_exhaustion() {
        let desired = (0..4).map(entry).collect::<Vec<_>>();
        let selected_url = desired[0].image_url.clone();
        let mut residency = PreviewResidency::new();
        let _ = residency.reconcile(&desired);
        let acquired = residency.handle(
            PreviewEvent::Acquired {
                image_url: selected_url.clone(),
                result: Ok(PathBuf::from("selected.jpg")),
            },
            &desired,
        );
        assert!(acquired.commands.iter().any(|command| matches!(
            command,
            PreviewCommand::Allocate { image_url, .. } if image_url == &selected_url
        )));

        let first = residency.handle(
            PreviewEvent::AllocationFailed {
                image_url: selected_url.clone(),
                error: iced_image::Error::OutOfMemory,
            },
            &desired,
        );
        assert!(first.selected_failure.is_none());
        assert!(first.commands.iter().any(|command| matches!(
            command,
            PreviewCommand::Allocate { image_url, .. } if image_url == &selected_url
        )));

        let second = residency.handle(
            PreviewEvent::AllocationFailed {
                image_url: selected_url.clone(),
                error: iced_image::Error::OutOfMemory,
            },
            &desired,
        );
        assert!(matches!(
            second.selected_failure,
            Some(PreviewFailure::Allocation(iced_image::Error::OutOfMemory))
        ));
        assert!(!second.commands.iter().any(|command| matches!(
            command,
            PreviewCommand::Allocate { image_url, .. } if image_url == &selected_url
        )));
    }

    #[test]
    /// Verifies an invalid preview is removed before acquisition is retried.
    fn invalidates_and_reacquires_an_empty_preview() {
        let desired = vec![entry(0)];
        let image_url = desired[0].image_url.clone();
        let mut residency = PreviewResidency::new();
        let _ = residency.reconcile(&desired);
        let _ = residency.handle(
            PreviewEvent::Acquired {
                image_url: image_url.clone(),
                result: Ok(PathBuf::from("empty.jpg")),
            },
            &desired,
        );

        let invalid = residency.handle(
            PreviewEvent::AllocationFailed {
                image_url: image_url.clone(),
                error: iced_image::Error::Empty,
            },
            &desired,
        );
        assert!(invalid.selected_failure.is_none());
        assert_eq!(
            invalid.commands,
            vec![PreviewCommand::RemoveInvalid {
                image_url: image_url.clone(),
                path: Some(PathBuf::from("empty.jpg")),
            }]
        );

        let recovered = residency.handle(
            PreviewEvent::Invalidated {
                image_url,
                result: Ok(()),
            },
            &desired,
        );
        assert_eq!(
            recovered.commands,
            vec![PreviewCommand::Acquire(desired[0].clone())]
        );
    }
}
