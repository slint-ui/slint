const path = require("path");
const WasmPackPlugin = require("@wasm-tool/wasm-pack-plugin");
const CopyPlugin = require("copy-webpack-plugin");
const dist = path.resolve(__dirname, "dist");

module.exports = {
    mode: "production",
    entry: { index: "./index.js" },
    output: { path: dist, filename: "[name].js" },
    devServer: {
        contentBase: dist,
    },
    plugins: [
        new CopyPlugin({
            patterns: [
                { from: path.resolve(__dirname, "static") },
            ],
        }),
        new WasmPackPlugin({
            crateDirectory: __dirname,
        }),
    ]
};
