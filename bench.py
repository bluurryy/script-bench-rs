import subprocess

def run_benchmark(feature):
    print(f"Running benchmark for koto {feature}")

    with open(f"out/{feature}", "w") as f:
        subprocess.run(
            f"cargo criterion --bench koto --message-format json --features {feature}",
            shell=True,
            stdout=f,
            stderr=subprocess.DEVNULL,
            text=True,
            check=True,
        )

features = ["rc", "arc", "gc", "agc"]

for feature in features:
    run_benchmark(feature)