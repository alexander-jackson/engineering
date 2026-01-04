import { defineConfig } from '@rsbuild/core';
import { pluginReact } from '@rsbuild/plugin-react';
import { pluginSvgr } from '@rsbuild/plugin-svgr';

export default defineConfig({
  plugins: [
    pluginReact(),
    pluginSvgr(),
  ],

  source: {
    entry: {
      index: './src/index.tsx',
    },
  },

  resolve: {
    // Critical: Preserve ~ → src/ alias
    alias: {
      '~': './src',
    },
  },

  html: {
    template: './public/index.html',
    templateParameters: {
      PUBLIC_URL: '',
    },
  },

  output: {
    // Match current Webpack output structure
    distPath: {
      root: 'build',
      js: 'static/js',
      css: 'static/css',
      svg: 'static/media',
      font: 'static/media',
      image: 'static/media',
      media: 'static/media',
    },
    filename: {
      js: '[name].[contenthash:8].js',
      css: '[name].[contenthash:8].css',
    },
    sourceMap: {
      js: process.env.NODE_ENV === 'production'
        ? (process.env.GENERATE_SOURCEMAP !== 'false' ? 'source-map' : false)
        : 'cheap-module-source-map',
      css: true,
    },
  },

  server: {
    port: 3000,
    // Critical: CORS headers for backend API calls
    headers: {
      'Access-Control-Allow-Origin': '*',
      'Access-Control-Allow-Methods': '*',
      'Access-Control-Allow-Headers': '*',
    },
    historyApiFallback: true,
  },

  tools: {
    postcss: (config) => {
      config.postcssOptions = {
        plugins: [
          'postcss-flexbugs-fixes',
          [
            'postcss-preset-env',
            {
              autoprefixer: {
                flexbox: 'no-2009',
              },
              stage: 3,
            },
          ],
          'postcss-normalize',
        ],
      };
      return config;
    },
  },

  performance: {
    chunkSplit: {
      strategy: 'split-by-experience',
    },
  },
});
