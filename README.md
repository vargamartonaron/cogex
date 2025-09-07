# Cognitive Experiment Application

A high-precision cognitive experiment platform built with Rust, targeting sub-millisecond timing accuracy for psychological and neuroscience research.

## Features

- **Sub-millisecond timing precision** using platform-specific high-resolution timers
- **Cross-platform support** (Windows, Linux, macOS) with platform-optimized implementations
- **Hardware-accelerated rendering** using pixels + tiny-skia for 2D graphics
- **Structured experiment flow**: Welcome → Practice → Experiment → Debrief
- **Precise stimulus control** with configurable timing parameters
- **Real-time performance monitoring** with frame timing analysis
- **Data export** in JSON format for analysis

## Architecture

### Core Components

1. **Timer Module** (`src/timer.rs`)
   - Platform-specific high-precision timers
   - Windows: Waitable timers with 100ns resolution
   - Linux: `clock_nanosleep` with CLOCK_MONOTONIC
   - macOS: `mach_absolute_time` with busy-wait for ultra-precision
   - Hardware timestamp counter support (x86_64)

2. **Experiment Module** (`src/experiment.rs`)
   - State management for experiment phases
   - Trial generation and stimulus randomization  
   - Response time measurement and data collection
   - Results analysis and export

3. **Renderer Module** (`src/renderer.rs`)
   - Hardware-accelerated 2D rendering using tiny-skia
   - Optimized stimulus presentation (circles, rectangles, arrows, text)
   - Real-time visual feedback

4. **Main Application** (`src/main.rs`)
   - Winit 0.30 event loop with ApplicationHandler trait
   - Precise frame timing control (144 Hz target)
   - Input handling and state coordination

### Timing Architecture

The application implements a multi-layered timing system:

- **Frame-level timing**: 144 Hz refresh rate with frame time monitoring
- **Stimulus timing**: Precise onset/offset control using hardware timers
- **Response timing**: Nanosecond-precision reaction time measurement
- **Jitter analysis**: Real-time monitoring of timing variability

## Usage

### Building and Running

```bash
# Clone the repository
git clone <repository-url>
cd cognitive-experiment

# Build for your platform
cargo build --release

# Run the experiment
cargo run --release
```

### Controls

- **SPACE**: Progress through experiment phases and respond to stimuli
- **ESC**: Emergency exit

### Configuration

Modify `ExperimentConfig` in `src/experiment.rs`:

```rust
ExperimentConfig {
    practice_trial_count: 20,        // Number of practice trials
    experiment_trial_count: 100,     // Main experiment trials
    fixation_duration_range: (500, 1500), // Fixation time (ms)
    stimulus_duration: 200,          // Stimulus presentation (ms)
    response_window: 2000,           // Response timeout (ms)
    feedback_duration: 500,          // Feedback display (ms)
    inter_trial_interval: 1000,     // Between trials (ms)
}
```

## Technical Specifications

### Timing Precision

- **Windows**: 100-nanosecond resolution using waitable timers
- **Linux**: Nanosecond resolution using `clock_nanosleep`
- **macOS**: Hardware-level precision using `mach_absolute_time`
- **Target latency**: <1ms stimulus presentation to photon

### Performance Targets

- Frame rate: 144 Hz (6.94ms frame time)
- Input latency: <0.5ms
- Stimulus precision: ±100μs
- Memory usage: <50MB
- CPU usage: <10% (single core)

### Platform-Specific Optimizations

#### Windows
- High-resolution multimedia timers
- Process priority boosting
- Thread affinity optimization
- DWM bypass for reduced latency

#### Linux
- Real-time scheduling policies
- CPU isolation techniques
- Hardware timestamp counters
- Memory lock optimization

#### macOS
- Core Animation bypassing
- Metal performance shaders
- Thread QoS optimization
- CVDisplayLink synchronization

## Data Output

Results are automatically saved as JSON:

```json
{
  "trial_id": 1,
  "stimulus_type": "Circle",
  "reaction_time_ns": 345234567,
  "response_correct": true,
  "timestamp": 1234567890123
}
```

### Analysis Fields

- `reaction_time_ns`: Nanosecond-precision reaction time
- `stimulus_type`: Type of stimulus presented
- `response_correct`: Response accuracy
- `timestamp`: Absolute timestamp for temporal analysis

## Research Applications

This platform is designed for experiments requiring precise timing:

- **Reaction time studies**
- **Temporal perception research**  
- **Psychophysics experiments**
- **Cognitive load assessment**
- **Attention and vigilance tasks**
- **Motor response timing**

## Dependencies

- `winit 0.30`: Cross-platform windowing
- `pixels 0.13`: Hardware-accelerated pixel buffer
- `tiny-skia 0.11`: High-quality 2D rendering
- `anyhow 1.0`: Error handling
- Platform-specific timer libraries

## Development

### Adding New Stimuli

Extend the `StimulusType` enum in `src/experiment.rs`:

```rust
pub enum StimulusType {
    // Existing types...
    CustomStimulus { 
        parameters: CustomParams,
        color: [u8; 4] 
    },
}
```

Implement rendering in `src/renderer.rs`:

```rust
fn render_stimulus(&self, pixmap: &mut Pixmap, stimulus: &StimulusType, position: (f32, f32)) -> Result<()> {
    match stimulus {
        StimulusType::CustomStimulus { parameters, color } => {
            self.draw_custom(pixmap, position, parameters, *color)?;
        }
        // Other cases...
    }
    Ok(())
}
```

### Platform Porting

To add new platform support:

1. Add platform detection in `src/timer.rs`
2. Implement `high_precision_sleep` for the platform
3. Add platform-specific dependencies in `Cargo.toml`
4. Test timing precision with hardware validation

## Validation

The platform includes built-in timing validation:

```bash
cargo run --release --features validation
```

This enables:
- Frame time histogram logging
- Latency distribution analysis
- Hardware timer calibration
- Jitter measurement and reporting

## License

MIT License - see LICENSE file for details.

## Contributing

1. Fork the repository
2. Create feature branch
3. Add tests for timing-critical code
4. Validate on target platforms
5. Submit pull request

## References

- [High-Precision Timing in Cognitive Experiments](link)
- [Platform-Specific Timer APIs](link)
- [Winit 0.30 Migration Guide](link)
- [Cognitive Experiment Design Principles](link)
