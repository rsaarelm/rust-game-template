#![feature(test)]
extern crate test;

use rand::Rng;
use test::Bencher;

use util::{AxisBox, Grid};

trait Space<B> {
    fn iter<'a>(&'a self) -> impl Iterator<Item = (usize, &'a B)>
    where
        B: 'a;
    fn insert(&mut self, bounds: B) -> usize;
    fn remove(&mut self, h: usize);
    fn update(&mut self, h: usize, bounds: B);
    fn intersecting<'a>(
        &'a self,
        bounds: &'a B,
    ) -> impl Iterator<Item = (usize, &'a B)>;
}

#[derive(Default)]
struct NaiveGrid<const N: usize> {
    items: Vec<Option<AxisBox<i32, N>>>,
    free_indices: Vec<usize>,
}

impl<const N: usize> Space<AxisBox<i32, N>> for NaiveGrid<N> {
    fn iter<'a>(&'a self) -> impl Iterator<Item = (usize, &'a AxisBox<i32, N>)>
    where
        AxisBox<i32, N>: 'a,
    {
        self.items
            .iter()
            .enumerate()
            .filter_map(|(i, b)| b.as_ref().map(|b| (i, b)))
    }

    fn insert(&mut self, bounds: AxisBox<i32, N>) -> usize {
        if let Some(reuse) = self.free_indices.pop() {
            self.items[reuse] = Some(bounds);
            reuse
        } else {
            self.items.push(Some(bounds));
            self.items.len() - 1
        }
    }

    fn remove(&mut self, h: usize) {
        self.items[h] = None;
        self.free_indices.push(h);
    }

    fn update(&mut self, h: usize, bounds: AxisBox<i32, N>) {
        self.items[h] = Some(bounds);
    }

    fn intersecting<'a>(
        &'a self,
        bounds: &'a AxisBox<i32, N>,
    ) -> impl Iterator<Item = (usize, &'a AxisBox<i32, N>)> {
        self.items
            .iter()
            .enumerate()
            .filter_map(|(i, b)| match b.as_ref() {
                Some(b) if b.intersects(bounds) => Some((i, b)),
                _ => None,
            })
    }
}

impl<const N: usize> Space<AxisBox<i32, N>> for Grid<i32, N, AxisBox<i32, N>> {
    fn iter<'a>(&'a self) -> impl Iterator<Item = (usize, &'a AxisBox<i32, N>)>
    where
        AxisBox<i32, N>: 'a,
    {
        Grid::iter(self)
    }

    fn insert(&mut self, bounds: AxisBox<i32, N>) -> usize {
        Grid::insert(self, bounds)
    }

    fn remove(&mut self, h: usize) {
        Grid::remove(self, h)
    }

    fn update(&mut self, h: usize, bounds: AxisBox<i32, N>) {
        Grid::update(self, h, |old_bounds| *old_bounds = bounds);
    }

    fn intersecting<'a>(
        &'a self,
        bounds: &'a AxisBox<i32, N>,
    ) -> impl Iterator<Item = (usize, &'a AxisBox<i32, N>)> {
        Grid::intersecting(self, bounds)
    }
}

fn fill(rng: &mut impl Rng, s: &mut impl Space<AxisBox<i32, 2>>) {
    for _ in 0..1000 {
        let x = rng.random_range(0..1024);
        let y = rng.random_range(0..1024);
        let w = rng.random_range(1..12);
        let h = rng.random_range(1..12);

        s.insert(AxisBox::new([x, y], [x + w, y + h]));
    }
}

// Test query and delete.
fn sweep(s: &mut impl Space<AxisBox<i32, 2>>) {
    fill(&mut util::srng(&123), s);
    for x in 0..1024 {
        let kill_list: Vec<usize> = s
            .intersecting(&AxisBox::new([x, 502], [x + 2, 522]))
            .map(|(i, _)| i)
            .collect();
        for i in kill_list {
            s.remove(i);
        }
    }
}

// Test update
fn perturb(s: &mut impl Space<AxisBox<i32, 2>>) {
    let mut rng = util::srng(&123);

    fill(&mut rng, s);

    for _ in 0..16 {
        let updates: Vec<(usize, AxisBox<i32, 2>)> = s
            .iter()
            .map(|(i, b)| {
                let [x, y] = b.min();
                let [x, y] = [
                    x + rng.random_range(-8..=8),
                    y + rng.random_range(-8..=8),
                ];
                let [sx, sy] = b.dim();
                (i, AxisBox::new([x, y], [x + sx, y + sy]))
            })
            .collect();
        for (i, a) in updates {
            s.update(i, a);
        }
    }
}

// Test several cell sizes to try to find the sweet spot for efficient updates
// and queries.

#[rustfmt::skip] #[bench] fn sweep_grid_8(b: &mut Bencher) { b.iter(|| { sweep(&mut Grid::new([8, 8])); }) }
#[rustfmt::skip] #[bench] fn sweep_grid_16(b: &mut Bencher) { b.iter(|| { sweep(&mut Grid::new([16, 16])); }) }
#[rustfmt::skip] #[bench] fn sweep_grid_32(b: &mut Bencher) { b.iter(|| { sweep(&mut Grid::new([32, 32])); }) }
#[rustfmt::skip] #[bench] fn sweep_grid_64(b: &mut Bencher) { b.iter(|| { sweep(&mut Grid::new([64, 64])); }) }
#[rustfmt::skip] #[bench] fn sweep_grid_128(b: &mut Bencher) { b.iter(|| { sweep(&mut Grid::new([128, 128])); }) }
#[rustfmt::skip] #[bench] fn sweep_reference_grid(b: &mut Bencher) { b.iter(|| { sweep(&mut NaiveGrid::default()); }) }

#[rustfmt::skip] #[bench] fn perturb_grid_8(b: &mut Bencher) { b.iter(|| { perturb(&mut Grid::new([8, 8])); }) }
#[rustfmt::skip] #[bench] fn perturb_grid_16(b: &mut Bencher) { b.iter(|| { perturb(&mut Grid::new([16, 16])); }) }
#[rustfmt::skip] #[bench] fn perturb_grid_32(b: &mut Bencher) { b.iter(|| { perturb(&mut Grid::new([32, 32])); }) }
#[rustfmt::skip] #[bench] fn perturb_grid_64(b: &mut Bencher) { b.iter(|| { perturb(&mut Grid::new([64, 64])); }) }
#[rustfmt::skip] #[bench] fn perturb_grid_128(b: &mut Bencher) { b.iter(|| { perturb(&mut Grid::new([128, 128])); }) }
#[rustfmt::skip] #[bench] fn perturb_reference_grid(b: &mut Bencher) { b.iter(|| { perturb(&mut NaiveGrid::default()); })}
