//! Integration test for MPRIS server behavior using actual MprisManager.

use std::time::Duration;

use biu_tui::audio::PlayerState;
use biu_tui::mpris::MprisManager;
use biu_tui::playing_list::PlaylistItem;

#[test]
fn test_mpris_manager_metadata_and_status() {
    let mgr = MprisManager::new().expect("Failed to create MprisManager");

    // Wait for D-Bus registration
    std::thread::sleep(Duration::from_millis(500));

    // Check player is visible
    let output = std::process::Command::new("playerctl")
        .args(["--list-all"])
        .output()
        .expect("Failed to run playerctl");
    let players = String::from_utf8_lossy(&output.stdout);
    eprintln!("[TEST] Players listed: {}", players.trim());
    assert!(
        players.contains("com.github.biu-tui"),
        "Player should be visible on D-Bus"
    );

    // Create a test playlist item
    let item = PlaylistItem {
        bvid: "BV1test123abc".to_string(),
        cid: 12345,
        title: "Test Song Title 测试标题".to_string(),
        artist: "Test Artist 测试歌手".to_string(),
        duration: 180,
    };

    // Send track metadata (mimics play_playlist_item)
    mgr.set_track(&item);
    std::thread::sleep(Duration::from_millis(200));

    // Send Playing state (mimics play_playlist_item)
    mgr.set_state(PlayerState::Playing);
    std::thread::sleep(Duration::from_millis(500));

    // Query metadata via playerctl
    let output = std::process::Command::new("playerctl")
        .args(["--player=com.github.biu-tui", "metadata"])
        .output()
        .expect("Failed to run playerctl");
    let metadata_str = String::from_utf8_lossy(&output.stdout);
    let metadata_err = String::from_utf8_lossy(&output.stderr);
    eprintln!("[TEST] Metadata after Playing:");
    eprintln!("  stdout: '{}'", metadata_str.trim());
    eprintln!("  stderr: '{}'", metadata_err.trim());

    // Query status via playerctl
    let output = std::process::Command::new("playerctl")
        .args(["--player=com.github.biu-tui", "status"])
        .output()
        .expect("Failed to run playerctl");
    let status = String::from_utf8_lossy(&output.stdout);
    eprintln!("[TEST] Status: '{}'", status.trim());

    // Now send position updates like the app does (flood test)
    for _ in 0..20 {
        mgr.set_position(Duration::from_secs(5));
        std::thread::sleep(Duration::from_millis(100));
    }

    // Query metadata again after position update flood
    let output = std::process::Command::new("playerctl")
        .args(["--player=com.github.biu-tui", "metadata"])
        .output()
        .expect("Failed to run playerctl");
    let metadata_str2 = String::from_utf8_lossy(&output.stdout);
    eprintln!(
        "[TEST] Metadata after position flood: '{}'",
        metadata_str2.trim()
    );

    // Verify metadata contains expected values
    assert!(
        metadata_str.contains("Test Song Title"),
        "Metadata should contain title. Got: {}",
        metadata_str
    );
    assert!(
        metadata_str.contains("Test Artist"),
        "Metadata should contain artist. Got: {}",
        metadata_str
    );
    assert_eq!(status.trim(), "Playing", "Status should be Playing");

    // Also verify metadata survived the position flood
    assert!(
        metadata_str2.contains("Test Song Title"),
        "Metadata should survive position flood. Got: {}",
        metadata_str2
    );
}
