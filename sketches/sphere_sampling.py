import rerun as rr
import rerun.blueprint as rrb
import numpy as np

rr.init("terrain_and_stuff_sketches", spawn=True)
rr.send_blueprint(
    rrb.Blueprint(
        rrb.Vertical(
            rrb.Horizontal(
                rrb.Spatial3DView(origin="pcg_1024"),
                rrb.Spatial3DView(origin="np_random_1024"),
                rrb.Spatial3DView(origin="halton_1024"),
            ),
            rrb.Horizontal(
                rrb.Spatial3DView(origin="pcg_2048"),
                rrb.Spatial3DView(origin="np_random_2048"),
                rrb.Spatial3DView(origin="halton_2048"),
            ),
        ),
        collapse_panels=True,
    )
)

NUM_SAMPLES = 1024


def halton(n, base):
    """Generate Halton sequence for z and t"""
    h = np.zeros(n)
    for i in range(n):
        f = 1
        r = 0
        j = i + 1
        while j > 0:
            f = f / base
            r = r + f * (j % base)
            j = j // base
        h[i] = r
    return h


def pcg_hash(input):
    """See https://www.reedbeta.com/blog/hash-functions-for-gpu-rendering/"""
    state = (input * 747796405 + 2891336453) & 0xFFFFFFFF
    word = (((state >> ((state >> 28) + 4)) ^ state) * 277803737) & 0xFFFFFFFF
    return ((word >> 22) ^ word) & 0xFFFFFFFF


pcg_state = 123


def pcg_prng():
    global pcg_state
    pcg_state = pcg_hash(pcg_state)
    return float(pcg_state) / 0xFFFFFFFF


def pcg_vars(num_samples):
    np.linspace(0, num_samples * 2)
    z = np.zeros(num_samples)
    t = np.zeros(num_samples)
    for i in range(num_samples):
        z[i] = pcg_prng()
        t[i] = pcg_prng()
    z = 2 * z - 1  # Map [0,1] to [-1,1]
    t = 2 * t * np.pi  # Map [0,1] to [0, 2π]
    return z, t


def np_random_vars(num_samples):
    z = np.random.uniform(-1, 1, size=num_samples)
    t = np.random.uniform(0, 2 * np.pi, size=num_samples)
    return z, t


def halton_vars(num_samples):
    z = 2 * halton(num_samples, 2) - 1  # Map [0,1] to [-1,1]
    t = 2 * np.pi * halton(num_samples, 3)  # Map [0,1] to [0, 2π]
    return z, t


def vars_to_points(z, t):
    r = np.sqrt(1 - z**2)
    x = r * np.cos(t)
    y = r * np.sin(t)
    return np.column_stack([x, y, z])


color = 0x00FFFFFF

for num_samples in [1024, 2048]:
    z, t = halton_vars(num_samples)
    points = vars_to_points(z, t)
    rr.log(
        f"halton_{num_samples}",
        rr.Points3D(
            positions=points,
            colors=color,
        ),
    )

    z, t = pcg_vars(num_samples)
    points = vars_to_points(z, t)
    rr.log(
        f"pcg_{num_samples}",
        rr.Points3D(positions=points, colors=color),
    )

    z, t = np_random_vars(num_samples)
    points = vars_to_points(z, t)
    rr.log(
        f"np_random_{num_samples}",
        rr.Points3D(positions=points, colors=color),
    )
