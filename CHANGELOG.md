# Changelog

What changed for people using Patina, newest first. Code-only work stays in the git history. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and versions follow [SemVer](https://semver.org).

## [Unreleased]

### Added
- A MIDI Control Center template (`docs/Patina.minilabmk2`), so setting up the MiniLab is one import instead of about 40 controls by hand.

### Changed
- Patina only listens to the MiniLab now and never writes to it. Store the setup in one of the device's memories with MIDI Control Center; the pad lights are gone.
- The release download is now `patina-<version>.zip`, with a `SHA256SUMS` file next to it.

## [1.1.2] - 2026-11-09

### Fixed
- The EQ no longer breaks into silence or noise at very low sample rates.

## [1.1.1] - 2026-11-09

### Fixed
- On Windows, unloading Patina could crash the host.

## [1.1.0] - 2026-11-09

### Changed
- Patina now runs on nice-plug instead of nih-plug.

### Fixed
- The comb filter could crash the host with some parameter values.

## [1.0.0] - 2026-11-09

### Added
- Patina, a polysynth plugin (VST3 + CLAP) for macOS and Windows: two oscillators with FM, ring, unison, sub and noise, filters, envelopes, three LFOs, and effects from distortion and bitcrush to chorus, delay, reverb, EQ, compression, width and a limiter.
- Tempo-synced LFOs and delay, and an arpeggiator with seven modes, swing, repeats and latch.
- A factory bank with categories, tags, search, favorites and eight named macros per preset, plus user presets with compare.
- Seven pages that mirror an Arturia MiniLab MK2's 16 knobs, with relative encoders, fine adjust while knob 1 or 9 is held, pad page switching and preset browsing from pad 8.
- Audition mode (Shift A) for rating presets.
- A Mac app, the same synth without a DAW, built from source with `scripts/install.sh`.
