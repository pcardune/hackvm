use criterion::{black_box, criterion_group, criterion_main, Criterion};
use hackvm::{VMEmulator, VMProgram};

pub fn criterion_benchmark(c: &mut Criterion) {
    let program = VMProgram::new(&vec![(
        "Sys.vm",
        "
        function Sys.init 0
            push constant 10
            pop static 0
            label LOOP
            call Sys.incr 0
            pop temp 0
            goto LOOP
        return
        
        function Sys.incr 0
            push static 0
            push constant 1
            add
            pop static 0
            push static 0
        return
        ",
    )])
    .unwrap();

    c.bench_function("fib 20", |b| {
        b.iter(|| VMEmulator::new(program.clone()).run(black_box(2000)))
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
