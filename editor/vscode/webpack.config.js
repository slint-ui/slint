'use strict';

const path = require('path');
const webpack = require('webpack');

const WasmPackPlugin = require("@wasm-tool/wasm-pack-plugin");


/** @typedef {import('webpack').Configuration} WebpackConfig **/
/** @type WebpackConfig */
const webExtensionConfig = {
    mode: 'none',
    target: 'webworker', // extensions run in a webworker context
    entry: {
        browser: './src/browser.ts', // source of the web extension main file
    },
    output: {
        filename: '[name].js',
        path: path.join(__dirname, 'out'),
        libraryTarget: 'commonjs',
        devtoolModuleFilenameTemplate: '../[resource-path]'
    },
    resolve: {
        mainFields: ['browser', 'module', 'main'], // look for `browser` entry point in imported node modules
        extensions: ['.ts', '.js'], // support ts-files and js-files
        fallback: {
            path: require.resolve('path-browserify')
        },
    },
    module: {
        rules: [
            {
                test: /\.ts$/,
                exclude: /node_modules/,
                use: [
                    {
                        loader: 'ts-loader'
                    }
                ]
            }
        ]
    },
    externals: {
        vscode: 'commonjs vscode' // ignored because it doesn't exist
    },
    performance: {
        hints: false
    },
    devtool: 'nosources-source-map' // create a source map that points to the original source file
};

/** @type WebpackConfig */
const browserServerConfig = {
    context: path.join(__dirname, 'browser-language-server'),
    mode: 'none',
    target: 'webworker', // web extensions run in a webworker context
    entry: {
        browserServerMain: './browserServerMain.ts',
    },
    output: {
        filename: '[name].js',
        path: path.join(__dirname, 'out'),
        libraryTarget: 'var',
        library: 'serverExportVar',
        devtoolModuleFilenameTemplate: '../[resource-path]'
    },
    resolve: {
        mainFields: ['module', 'main'],
        extensions: ['.ts', '.js'], // support ts-files and js-files
        fallback: {
            path: require.resolve('path-browserify')
        },
    },
    module: {
        rules: [
            {
                test: /\.ts$/,
                exclude: /node_modules/,
                use: [
                    {
                        loader: 'ts-loader',
                    },
                ],
            },
        ],
    },
    plugins: [
        new WasmPackPlugin({
            crateDirectory: path.resolve(__dirname, "../../tools/lsp"),
        }),
    ],
    experiments: {
        asyncWebAssembly: true,
    },
    externals: {
        vscode: 'commonjs vscode', // ignored because it doesn't exist
    },
    performance: {
        hints: false,
    },
    devtool: 'nosources-source-map',
};


module.exports = [webExtensionConfig, browserServerConfig];
