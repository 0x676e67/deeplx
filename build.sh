#!/bin/bash

set -e

root=$(pwd)
: ${tag=latest}
: ${rmi=false}
: ${os=linux}
[ ! -d bin ] && mkdir bin

# Separate arrays for target architectures and Docker images
target_architectures=(
    "x86_64-unknown-linux-musl"
    "aarch64-unknown-linux-musl"
    "armv7-unknown-linux-musleabi"
    "armv7-unknown-linux-musleabihf"
    "arm-unknown-linux-musleabi"
    "arm-unknown-linux-musleabihf"
    "armv5te-unknown-linux-musleabi"
    "i686-unknown-linux-gnu"
    "x86_64-pc-windows-msvc"
)

pull_docker_image() {
    image="ghcr.io/gngpp/rust-musl-cross:$1"
    echo "Pulling $image"
    docker pull $image
}

rmi_docker_image() {
    image="ghcr.io/gngpp/rust-musl-cross:$1"
    echo "Removing $image docker image"
    if [ "$rmi" = "true" ]; then
        docker rmi $image
    fi
}

build_macos_target() {
    echo "Building $1"
    cargo build --release --target $1 --features mimalloc
    sudo chmod -R 777 target
    cd target/$1/release
    tar czvf deeplx-$tag-$1.tar.gz deeplx
    shasum -a 256 deeplx-$tag-$1.tar.gz >deeplx-$tag-$1.tar.gz.sha256
    mv deeplx-$tag-$1.tar.gz $root/bin/
    mv deeplx-$tag-$1.tar.gz.sha256 $root/bin/
    cd -
}

build_linux_target() {
    docker_image="ghcr.io/gngpp/rust-musl-cross:$1"

    features=""
    if [ "$1" = "armv5te-unknown-linux-musleabi" ] || [ "$1" = "arm-unknown-linux-musleabi" ] || [ "$1" = "arm-unknown-linux-musleabihf" ]; then
        features="--features rpmalloc"
    else
        if [ "$1" = "i686-unknown-linux-gnu" ]; then
            features=""
        else
            features="--features mimalloc"
        fi
    fi

    echo "Building $1"
    docker run --rm -t --user=$UID:$(id -g $USER) \
        -v $(pwd):/home/rust/src \
        -v $HOME/.cargo/registry:/root/.cargo/registry \
        -v $HOME/.cargo/git:/root/.cargo/git \
        -e "FEATURES=$features" \
        -e "TARGET=$1" \
        $docker_image /bin/bash -c "cargo build --release --target \$TARGET  \$FEATURES"

    sudo chmod -R 777 target
    if [ "$1" != "i686-unknown-linux-gnu" ]; then
        upx --best --lzma target/$1/release/deeplx
    fi
    cd target/$1/release
    tar czvf deeplx-$tag-$1.tar.gz deeplx
    shasum -a 256 deeplx-$tag-$1.tar.gz >deeplx-$tag-$1.tar.gz.sha256
    mv deeplx-$tag-$1.tar.gz $root/bin/
    mv deeplx-$tag-$1.tar.gz.sha256 $root/bin/
    cd -
}

build_windows_target() {
    docker_image="ghcr.io/gngpp/rust-musl-cross:$1"

    echo "Building $1"
    docker run --rm -t \
        -v $(pwd):/home/rust/src \
        -v $HOME/.cargo/registry:/usr/local/cargo/registry \
        -v $HOME/.cargo/git:/usr/local/cargo/git \
        $docker_image cargo xwin build --release --target $1

    sudo chmod -R 777 target
    upx --best --lzma target/$1/release/deeplx.exe
    cd target/$1/release
    tar czvf deeplx-$tag-$1.tar.gz deeplx.exe
    shasum -a 256 deeplx-$tag-$1.tar.gz >deeplx-$tag-$1.tar.gz.sha256
    mv deeplx-$tag-$1.tar.gz $root/bin/
    mv deeplx-$tag-$1.tar.gz.sha256 $root/bin/
    cd -
}

if [ "$os" = "windows" ]; then
    target="x86_64-pc-windows-msvc"
    pull_docker_image "$target"
    build_windows_target "$target"
    rmi_docker_image "$target"
fi

if [ "$os" = "linux" ]; then
    target_list=(
        "x86_64-unknown-linux-musl"
        "aarch64-unknown-linux-musl"
        "armv7-unknown-linux-musleabi"
        "armv7-unknown-linux-musleabihf"
        "armv5te-unknown-linux-musleabi"
        "arm-unknown-linux-musleabi"
        "arm-unknown-linux-musleabihf"
        "i686-unknown-linux-gnu"
    )

    for target in "${target_list[@]}"; do
        pull_docker_image "$target"
        build_linux_target "$target"
        rmi_docker_image "$target"
    done
fi

if [ "$os" = "macos" ]; then
    target_list=(
        "x86_64-apple-darwin"
        "aarch64-apple-darwin"
    )
    for target in "${target_list[@]}"; do
        echo "Adding $target to the build queue"
        rustup target add "$target"
        build_macos_target "$target"
    done
fi

generate_directory_tree() {
    find "$1" -print | sed -e 's;[^/]*/;|____;g;s;____|; |;g'
}

generate_directory_tree "bin"