# Infinite Scroll for Favorite Folders and History

## Overview

Implement infinite scroll functionality for favorite folder contents and history page. When the user navigates to the last item in the list, automatically load more items from the API.

## Requirements

- Load more songs when cursor scrolls to the bottom of favorite folder contents
- Load more items when cursor scrolls to the bottom of history page
- Show "Loading more..." status message during load
- Handle errors gracefully without blocking the UI

## Design

### Data Model Changes

Add pagination state to `LibraryScreen`:

```rust
pub struct LibraryScreen {
    // Existing fields...
    
    // Pagination for favorite folder resources
    current_folder_page: u32,
    has_more_resources: bool,
    is_loading_more: bool,
    
    // Pagination for history
    history_page: u32,
    has_more_history: bool,
}
```

### Loading Trigger

Modify `next_item()` to check if we're at the last item and trigger loading:

```rust
pub fn next_item(
    &mut self, 
    playing_list: &Arc<Mutex<PlayingListManager>>,
    client: Arc<Mutex<BilibiliClient>>
) {
    let len = self.current_list_len_with_playlist(playing_list);
    if len > 0 {
        let current = self.list_state.selected().unwrap_or(0);
        let next_idx = if current >= len - 1 { 0 } else { current + 1 };
        self.list_state.select(Some(next_idx));
        
        // Trigger load more when reaching the last item
        if next_idx == len - 1 {
            self.try_load_more(client);
        }
    }
}
```

### Load More Implementation

```rust
fn try_load_more(&mut self, client: Arc<Mutex<BilibiliClient>>) {
    if self.is_loading_more {
        return;
    }
    
    match self.current_tab {
        LibraryTab::Favorites => {
            if let NavigationLevel::Videos { folder_id, .. } = &self.nav_level {
                self.try_load_more_favorites(client, *folder_id);
            }
        }
        LibraryTab::History => self.try_load_more_history(client),
        _ => {}
    }
}

fn try_load_more_favorites(&mut self, client: Arc<Mutex<BilibiliClient>>, folder_id: u64) {
    if !self.has_more_resources || self.is_loading_more {
        return;
    }
    
    self.is_loading_more = true;
    self.status_message = Some("Loading more...".to_string());
    
    let next_page = self.current_folder_page + 1;
    
    let result = {
        let client = client.lock();
        let rt = tokio::runtime::Runtime::new().ok();
        rt.and_then(|rt| rt.block_on(client.get_folder_resources(folder_id, next_page)).ok())
    };
    
    match result {
        Some((new_resources, has_more)) => {
            self.resources.extend(new_resources);
            self.current_folder_page = next_page;
            self.has_more_resources = has_more;
            self.status_message = None;
        }
        None => {
            self.status_message = Some("Failed to load more".to_string());
        }
    }
    
    self.is_loading_more = false;
}

fn try_load_more_history(&mut self, client: Arc<Mutex<BilibiliClient>>) {
    if !self.has_more_history || self.is_loading_more {
        return;
    }
    
    self.is_loading_more = true;
    self.status_message = Some("Loading more...".to_string());
    
    let next_page = self.history_page + 1;
    
    let result = {
        let client = client.lock();
        let rt = tokio::runtime::Runtime::new().ok();
        rt.and_then(|rt| rt.block_on(client.get_history(next_page)).ok())
    };
    
    match result {
        Some(new_history) => {
            // Infer has_more from returned count
            self.has_more_history = new_history.len() >= 20;
            self.history.extend(new_history);
            self.history_page = next_page;
            self.status_message = None;
        }
        None => {
            self.status_message = Some("Failed to load more".to_string());
        }
    }
    
    self.is_loading_more = false;
}
```

### State Reset

Reset pagination state when entering a new folder:

```rust
fn select_folder(&mut self, client: Arc<Mutex<BilibiliClient>>) -> anyhow::Result<()> {
    // ... existing code to get folder_id ...
    
    // Reset pagination for new folder
    self.current_folder_page = 1;
    self.has_more_resources = true;
    
    let resources = {
        let client = client.lock();
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(client.get_folder_resources(folder_id, 1))?
    };
    
    self.resources = resources.0;
    self.has_more_resources = resources.1;
    // ... rest of existing code
}
```

Initialize history pagination in `load_data()`:

```rust
pub fn load_data(&mut self, client: Arc<Mutex<BilibiliClient>>) -> anyhow::Result<()> {
    // ... existing code ...
    
    // Initialize history pagination
    self.history_page = 1;
    self.has_more_history = self.history.len() >= 20;
    
    // ... rest of existing code
}
```

### API Considerations

The `get_folder_resources` API already returns `(Vec<FavoriteResource>, bool)` where the bool indicates `has_more`.

The `get_history` API does not return `has_more`, so we infer it from the returned count:
- If 20 items returned, assume more exist
- If fewer than 20 items returned, we've reached the end

### Error Handling

- If loading fails, show error in `status_message` and keep `is_loading_more = false` so user can try again
- Don't block UI during load - the async call happens in a separate runtime
- Handle empty results gracefully

## Implementation Notes

1. The `next_item()` method signature will need to accept `client: Arc<Mutex<BilibiliClient>>`
2. The caller in `app.rs` will need to pass the client when calling `next_item()`
3. Default values in `LibraryScreen::new()`:
   - `current_folder_page: 1`
   - `has_more_resources: true`
   - `history_page: 1`
   - `has_more_history: true`
   - `is_loading_more: false`

## Testing

- Test that loading triggers when navigating to last item
- Test that loading doesn't trigger multiple times (lock mechanism)
- Test that pagination resets when entering a new folder
- Test error handling when API fails
- Test that has_more is correctly inferred for history
