const path = require('path');
const HtmlWebPackPlugin = require('html-webpack-plugin');
const WasmPackPlugin = require("@wasm-tool/wasm-pack-plugin");
const MonacoWebpackPlugin = require('monaco-editor-webpack-plugin');
const dist = path.resolve(__dirname, "dist");

module.exports = {
    entry: {
        app: './index.ts',
        'editor.worker': 'monaco-editor/esm/vs/editor/editor.worker.js',
        'json.worker': 'monaco-editor/esm/vs/language/json/json.worker',
        'css.worker': 'monaco-editor/esm/vs/language/css/css.worker',
        'html.worker': 'monaco-editor/esm/vs/language/html/html.worker',
        'ts.worker': 'monaco-editor/esm/vs/language/typescript/ts.worker'
    },
    resolve: {
        extensions: ['.ts', '.js']
    },
    output: {
        globalObject: 'self',
        filename: '[name].bundle.js',
        path: dist
    },
    module: {
        rules: [
            {
                test: /\.ts?$/,
                use: 'ts-loader',
                exclude: /node_modules/
            },
            {
                test: /\.css$/,
                use: ['style-loader', 'css-loader']
            },
            {
                test: /\.ttf$/,
                use: ['file-loader']
            },
        ]
    },
    plugins: [
        new HtmlWebPackPlugin({
            title: 'SixtyFPS Online Editor',
            template: 'index.html'
        }),
        new WasmPackPlugin({
            crateDirectory: path.resolve(__dirname, "../../api/sixtyfps-wasm-interpreter/"),
        }),
        new MonacoWebpackPlugin(),
    ]
};
