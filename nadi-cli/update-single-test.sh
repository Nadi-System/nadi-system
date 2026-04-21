filename=$1
../target/release/nadi $filename > "${filename%.tasks}.stdout"
