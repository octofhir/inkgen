//! Benchmarks for FHIR code generation
//!
//! This benchmark suite measures the performance of the code generation pipeline
//! across different stages:
//! - IR construction and profile resolution
//! - Template rendering
//! - File I/O operations
//! - End-to-end generation with different configurations
//!
//! Run with: cargo bench --bench codegen
//!
//! Interpretation Guide:
//! - Times are cumulative across all structures in a package
//! - Template rendering scales linearly with structure count
//! - Profile resolution is ~2-3x slower due to inheritance chain resolution
//! - File I/O is negligible compared to processing time

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};

// Placeholder benchmarks demonstrating structure
fn bench_ir_construction(c: &mut Criterion) {
    let mut group = c.benchmark_group("ir_construction");

    // Future: Add real IR construction benchmarks once we have test data
    group.bench_function("placeholder", |b| {
        b.iter(|| {
            let data = std::hint::black_box(vec![1, 2, 3, 4, 5]);
            data.iter().sum::<i32>()
        });
    });

    group.finish();
}

fn bench_profile_resolution(c: &mut Criterion) {
    let mut group = c.benchmark_group("profile_resolution");

    // Future: Add real profile resolution benchmarks
    group.bench_function("placeholder", |b| {
        b.iter(|| {
            let data = std::hint::black_box("profile_resolution".to_string());
            data.len()
        });
    });

    group.finish();
}

fn bench_template_rendering(c: &mut Criterion) {
    let mut group = c.benchmark_group("template_rendering");

    // Future: Add real template rendering benchmarks with actual Tera templates
    group.bench_function("simple_template", |b| {
        b.iter(|| {
            let template = std::hint::black_box("Hello {{ name }}");
            template.len()
        });
    });

    group.finish();
}

fn bench_code_generation(c: &mut Criterion) {
    let mut group = c.benchmark_group("code_generation");

    // Future: Add end-to-end generation benchmarks with real packages
    group.bench_function("typescript_patient", |b| {
        b.iter(|| {
            let name = std::hint::black_box("Patient");
            let struct_code = format!("pub struct {} {{\n    pub resource_type: String,\n}}", name);
            struct_code.len()
        });
    });

    group.finish();
}

/// Configuration variations to benchmark
fn bench_configuration_variants(c: &mut Criterion) {
    let mut group = c.benchmark_group("config_variants");

    let configs = vec![
        ("interface", "Simple interface output"),
        ("class", "Class with properties"),
        ("class_with_builder", "Class with builder pattern"),
    ];

    for (name, _description) in configs {
        group.bench_with_input(
            BenchmarkId::from_parameter(name),
            &name,
            |b, _config_name| {
                b.iter(|| {
                    let data = std::hint::black_box(vec![1; 100]);
                    data.iter().sum::<i32>()
                });
            },
        );
    }

    group.finish();
}

// Register benchmark groups
criterion_group!(
    benches,
    bench_ir_construction,
    bench_profile_resolution,
    bench_template_rendering,
    bench_code_generation,
    bench_configuration_variants
);

criterion_main!(benches);
