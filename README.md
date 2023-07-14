# morrigu-rs
Morrigu is a small(-ish) rendering framework created from scratch, [originally written in C++](https://github.com/TableauBits/Morrigu), and which uses the Vulkan API for rendering. Similar to the original, it was built to allow me to learn more about computer graphics (especially outside of academia, where implementation details and architectural choices are often left out), and more specifically, the rust programming language as well as the Vulkan API.

## How to build
Install rustup with the latest stable rust toolchain and cargo, shaderc, the vulkan SDK, and Vulkan validation layers, then use the following command:
```sh
cargo build
```

This will build the main library (named `morrigu-rs`). A binary project name `macha` is available (and is the way I do most of my testing of the features implemented in the library). To build this, you can instead enter the command:
```sh
cargo build -p macha
```

The result of both command should end up in the `target/debug` directory, at the root of the project.

## Gallery
In the future, I will try to add screenshots and gif to document the progress made, but for now, please refer to the original C++ version !
