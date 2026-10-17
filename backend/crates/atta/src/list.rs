use std::{
    collections::HashMap,
    hash::{BuildHasher, RandomState},
    sync::OnceLock,
};

use crate::{
    Admission, Entry, Error, Level, Listing, PageToken, Reach, Resolved, Resolver, Segment,
    ordering::name_order,
    ordinal::ordinals,
    resolve::{Folder, Located, Outcome},
};

/// The keys of the hash that breaks ties between cards, drawn once per process.
static CARD_ORDER_KEYS: OnceLock<RandomState> = OnceLock::new();

/// Where an entry sorts among entries of the same name: open entries first, by segment, then
/// cards by a keyed hash of their real path, so a card's order never reveals its key.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Tiebreak {
    Open(Segment),
    Card(u64),
}

/// One entry of a listing, with the segment it sits at in its folder and its [`Tiebreak`].
#[derive(Debug, Clone)]
struct Ranked {
    segment: Segment,
    entry: Entry,
    tiebreak: Tiebreak,
}

/// The keyed hash a card sorts by among cards of the same name.
fn card_order(real_path: &crate::RealPath) -> u64 {
    CARD_ORDER_KEYS
        .get_or_init(RandomState::new)
        .hash_one(real_path)
}

impl Resolver<'_> {
    /// One page of the resolved node's entries, starting at `page`: `None` unless the viewer can
    /// open the node and it is a directory, and the one not-found for a node resolved for another
    /// viewer. Entries the viewer can't see or the host refuses go before the sort and the cut,
    /// so a page is never short and a token hints at nothing that was left out. Logs its outcome.
    #[tracing::instrument(level = "debug", skip_all)]
    pub async fn list(
        &self,
        resolved: &Resolved,
        page: Option<PageToken>,
    ) -> Result<Option<Listing>, Error> {
        let listed = self.list_unlogged(resolved, page).await;
        let outcome = listed
            .as_ref()
            .map_or_else(Outcome::from, |_| Outcome::from(resolved));
        let outcome: &'static str = outcome.into();
        let entries = listed
            .as_ref()
            .ok()
            .and_then(Option::as_ref)
            .map(|listing| listing.entries.len());
        tracing::debug!(outcome, depth = resolved.crumbs().len(), entries, "listed");
        listed
    }

    /// [`Resolver::list`]'s page, without its log line.
    async fn list_unlogged(
        &self,
        resolved: &Resolved,
        page: Option<PageToken>,
    ) -> Result<Option<Listing>, Error> {
        if resolved.viewer != self.viewer {
            return Err(Error::NotFound);
        }
        let Some(folder) = &resolved.folder else {
            return Ok(None);
        };
        let ranked = self.numbered_entries(folder).await?;
        let offset = page.map_or(0, |token| token.offset());
        let page_size = self.caps.page_size.max(1);
        let end = offset.saturating_add(page_size);
        let next = (end < ranked.len()).then(|| PageToken::at(end));
        let entries = ranked
            .into_iter()
            .skip(offset)
            .take(page_size)
            .map(|ranked| ranked.entry)
            .collect();
        let listing = Listing { entries, next };
        Ok(Some(listing))
    }

    /// The resolved node and its crumbs, each numbered as the listing of the folder it was
    /// reached through numbers it: its parent on the Den path, listed with the same admission a
    /// listing uses. The root, and a node that listing leaves out, get no number. Run only on a
    /// walk that succeeded; each folder on the path is listed once.
    pub async fn tell_apart(&self, mut resolved: Resolved) -> Result<Resolved, Error> {
        if resolved.viewer != self.viewer {
            return Err(Error::NotFound);
        }
        let mut numbers = Vec::with_capacity(resolved.steps.len());
        for step in &resolved.steps {
            let listed = self.numbered_entries(&step.from).await?;
            let number = listed
                .into_iter()
                .find(|ranked| ranked.segment == step.segment)
                .and_then(|ranked| ranked.entry.ordinal);
            numbers.push(number);
        }
        let reached = resolved
            .crumbs
            .iter_mut()
            .skip(1)
            .chain(std::iter::once(&mut resolved.node));
        for (entry, number) in reached.zip(numbers) {
            entry.ordinal = number;
        }
        Ok(resolved)
    }

    /// Every entry of `folder` the viewer can see and the host admits, in listing order, each
    /// numbered among the entries of the same name.
    async fn numbered_entries(&self, folder: &Folder) -> Result<Vec<Ranked>, Error> {
        let mut ranked = self.visible_entries(folder).await?;
        ranked.sort_by(|left, right| {
            name_order(left.entry.name.as_ref(), right.entry.name.as_ref())
                .then_with(|| left.tiebreak.cmp(&right.tiebreak))
        });
        let named: Vec<(&str, &Tiebreak)> = ranked
            .iter()
            .map(|ranked| (ranked.entry.name.as_ref(), &ranked.tiebreak))
            .collect();
        let numbers = ordinals(&named);
        for (ranked, number) in ranked.iter_mut().zip(numbers) {
            ranked.entry.ordinal = number;
        }
        Ok(ranked)
    }

    /// Every entry of `folder` the viewer can see and the host admits, unsorted and unnumbered.
    async fn visible_entries(&self, folder: &Folder) -> Result<Vec<Ranked>, Error> {
        let children = self.host.children(&self.viewer, &folder.real_path).await?;
        let mounts = self.host.mounts(&self.viewer, &folder.real_path).await?;
        let mut uses = HashMap::<Segment, usize>::new();
        let segments = children
            .iter()
            .map(|found| &found.metadata.key)
            .chain(mounts.iter().map(|mount| &mount.segment));
        for segment in segments {
            *uses.entry(segment.clone()).or_default() += 1;
        }
        let unique = |segment: &Segment| uses.get(segment) == Some(&1);
        let mut located = Vec::new();
        for found in children {
            let key = found.metadata.key.clone();
            if !unique(&key) {
                continue;
            }
            if let Some(child) = self.check(&folder.real_path, &key, &folder.below, found) {
                located.push((key, child, false));
            }
        }
        for mount in mounts {
            if !unique(&mount.segment) {
                continue;
            }
            if let Some(target) = self.locate(&mount.target).await? {
                located.push((mount.segment, target, true));
            }
        }
        let mut ranked = Vec::new();
        for (segment, node, mount) in located {
            let admitted = self.admitted_entry(&folder.den_path, segment, &node, mount);
            if let Some(entry) = admitted {
                ranked.push(entry);
            }
        }
        Ok(ranked)
    }

    /// The entry for one located node of a listing, with its [`Tiebreak`], unless the viewer
    /// can't see it or the host refuses it.
    fn admitted_entry(
        &self,
        folder_den_path: &[Segment],
        segment: Segment,
        node: &Located,
        mount: bool,
    ) -> Option<Ranked> {
        if node.access.level == Level::Private {
            return None;
        }
        let admission = self.host.admit(
            &self.viewer,
            &node.path,
            &node.found.metadata,
            node.access,
            Reach::Listing,
        );
        if admission == Admission::Refuse {
            return None;
        }
        let tiebreak = if node.access.level == Level::Public {
            Tiebreak::Open(segment.clone())
        } else {
            Tiebreak::Card(card_order(&node.path))
        };
        let mut den_path = folder_den_path.to_vec();
        den_path.push(segment.clone());
        let entry = self.entry(node, den_path, mount);
        let ranked = Ranked {
            segment,
            entry,
            tiebreak,
        };
        Some(ranked)
    }
}

#[cfg(test)]
mod tests;
