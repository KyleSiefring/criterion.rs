use std::path::PathBuf;
use std::process::Child;
use std::cmp::Ordering;

use criterion_plot::prelude::*;
use stats::univariate::Sample;

use kde;
use report::{BenchmarkId, ValueType};

use itertools::Itertools;

use super::{debug_script, escape_underscores, scale_time};
use super::{DARK_BLUE, DEFAULT_FONT, KDE_POINTS, LINEWIDTH, POINT_SIZE, SIZE};
use AxisScale;

const NUM_COLORS: usize = 8;
static COMPARISON_COLORS: [Color; NUM_COLORS] = [
    Color::Rgb(178, 34, 34),
    Color::Rgb(46, 139, 87),
    Color::Rgb(0, 139, 139),
    Color::Rgb(255, 215, 0),
    Color::Rgb(0, 0, 139),
    Color::Rgb(220, 20, 60),
    Color::Rgb(139, 0, 139),
    Color::Rgb(0, 255, 127),
];

impl AxisScale {
    fn to_gnuplot(&self) -> Scale {
        match *self {
            AxisScale::Linear => Scale::Linear,
            AxisScale::Logarithmic => Scale::Logarithmic,
        }
    }
}

#[cfg_attr(feature = "cargo-clippy", allow(explicit_counter_loop))]
pub fn line_comparison(
    group_id: &str,
    all_curves: &[&(BenchmarkId, Vec<f64>)],
    path: &str,
    value_type: ValueType,
    axis_scale: AxisScale,
) -> Child {
    let path = PathBuf::from(path);
    let mut f = Figure::new();

    let input_suffix = match value_type {
        ValueType::Bytes => " Size (Bytes)",
        ValueType::Elements => " Size (Elements)",
        ValueType::Value => "",
    };

    f.set(Font(DEFAULT_FONT))
        .set(SIZE)
        .configure(Key, |k| {
            k.set(Justification::Left)
                .set(Order::SampleText)
                .set(Position::Outside(Vertical::Top, Horizontal::Right))
        })
        .set(Title(format!(
            "{}: Comparison",
            escape_underscores(group_id)
        )))
        .configure(Axis::BottomX, |a| {
            a.set(Label(format!("Input{}", input_suffix)))
                .set(axis_scale.to_gnuplot())
        });

    let mut max = 0.0;
    let mut i = 0;

    // This assumes the curves are sorted. It also assumes that the benchmark IDs all have numeric
    // values or throughputs and that value is sensible (ie. not a mix of bytes and elements
    // or whatnot)
    for (key, group) in &all_curves
        .into_iter()
        .group_by(|&&&(ref id, _)| &id.function_id)
    {
        let mut tuples: Vec<_> = group
            .into_iter()
            .map(|&&(ref id, ref sample)| {
                // Unwrap is fine here because it will only fail if the assumptions above are not true
                // ie. programmer error.
                let x = id.as_number().unwrap();
                let y = Sample::new(sample).mean();

                if y > max {
                    max = y;
                }

                (x, y)
            })
            .collect();
        tuples.sort_by(|&(ax, _), &(bx, _)| (ax.partial_cmp(&bx).unwrap_or(Ordering::Less)));
        let (xs, ys): (Vec<_>, Vec<_>) = tuples.into_iter().unzip();

        let function_name = key.as_ref()
            .map(|string| escape_underscores(string))
            .unwrap();

        f.plot(Lines { x: &xs, y: &ys }, |c| {
            c.set(LINEWIDTH)
                .set(Label(function_name))
                .set(LineType::Solid)
                .set(COMPARISON_COLORS[i % NUM_COLORS])
        }).plot(Points { x: &xs, y: &ys }, |p| {
            p.set(PointType::FilledCircle)
                .set(POINT_SIZE)
                .set(COMPARISON_COLORS[i % NUM_COLORS])
        });

        i += 1;
    }

    let (scale, prefix) = scale_time(max);

    f.configure(Axis::LeftY, |a| {
        a.configure(Grid::Major, |g| g.show())
            .configure(Grid::Minor, |g| g.hide())
            .set(Label(format!("Average time ({}s)", prefix)))
            .set(axis_scale.to_gnuplot())
            .set(ScaleFactor(scale))
    });

    debug_script(&path, &f);
    f.set(Output(path)).draw().unwrap()
}

pub fn violin(
    group_id: &str,
    all_curves: &[&(BenchmarkId, Vec<f64>)],
    path: &str,
    axis_scale: AxisScale,
) -> Child {
    let path = PathBuf::from(&path);
    let all_curves_vec = all_curves.iter().rev().map(|&t| t).collect::<Vec<_>>();
    let all_curves: &[&(BenchmarkId, Vec<f64>)] = &*all_curves_vec;

    let kdes = all_curves
        .iter()
        .map(|&&(_, ref sample)| {
            let (x, mut y) = kde::sweep(Sample::new(sample), KDE_POINTS, None);
            let y_max = Sample::new(&y).max();
            for y in y.iter_mut() {
                *y /= y_max;
            }

            (x, y)
        })
        .collect::<Vec<_>>();
    let mut xs = kdes.iter()
        .flat_map(|&(ref x, _)| x.iter())
        .filter(|&&x| x > 0.);
    let (mut min, mut max) = {
        let &first = xs.next().unwrap();
        (first, first)
    };
    for &e in xs {
        if e < min {
            min = e;
        } else if e > max {
            max = e;
        }
    }
    let (scale, prefix) = scale_time(max);

    let tics = || (0..).map(|x| (f64::from(x)) + 0.5);
    let size = Size(1280, 200 + (25 * all_curves.len()));
    let mut f = Figure::new();
    f.set(Font(DEFAULT_FONT))
        .set(size)
        .set(Title(format!(
            "{}: Violin plot",
            escape_underscores(group_id)
        )))
        .configure(Axis::BottomX, |a| {
            a.configure(Grid::Major, |g| g.show())
                .configure(Grid::Minor, |g| g.hide())
                .set(Label(format!("Average time ({}s)", prefix)))
                .set(axis_scale.to_gnuplot())
                .set(ScaleFactor(scale))
        })
        .configure(Axis::LeftY, |a| {
            a.set(Label("Input"))
                .set(Range::Limits(0., all_curves.len() as f64))
                .set(TicLabels {
                    positions: tics(),
                    labels: all_curves
                        .iter()
                        .map(|&&(ref id, _)| escape_underscores(id.id())),
                })
        });

    let mut is_first = true;
    for (i, &(ref x, ref y)) in kdes.iter().enumerate() {
        let i = i as f64 + 0.5;
        let y1 = y.iter().map(|&y| i + y * 0.5);
        let y2 = y.iter().map(|&y| i - y * 0.5);

        f.plot(
            FilledCurve {
                x: &**x,
                y1: y1,
                y2: y2,
            },
            |c| {
                if is_first {
                    is_first = false;

                    c.set(DARK_BLUE).set(Label("PDF")).set(Opacity(0.25))
                } else {
                    c.set(DARK_BLUE).set(Opacity(0.25))
                }
            },
        );
    }
    debug_script(&path, &f);
    f.set(Output(path)).draw().unwrap()
}