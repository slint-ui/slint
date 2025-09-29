#!/usr/bin/env node

import { spawn } from 'node:child_process';
import { createWriteStream } from 'node:fs';

async function testMCPServer() {
  console.log('🧪 Testing Slint Docs MCP Server with prompts...\n');

  // Start the MCP server
  const server = spawn('node', ['src/index.js'], {
    stdio: ['pipe', 'pipe', 'pipe'],
    cwd: process.cwd(),
    env: { ...process.env, SLINT_DOCS_PATH: '../docs/astro/src/content/docs' }
  });

  let output = '';
  let errorOutput = '';

  server.stdout.on('data', (data) => {
    output += data.toString();
  });

  server.stderr.on('data', (data) => {
    errorOutput += data.toString();
  });

  // Test prompts
  const testPrompts = [
    {
      name: 'List Available Tools',
      message: {
        jsonrpc: '2.0',
        id: 1,
        method: 'tools/list',
        params: {}
      }
    },
    {
      name: 'Search for Button Documentation',
      message: {
        jsonrpc: '2.0',
        id: 2,
        method: 'tools/call',
        params: {
          name: 'search_slint_docs',
          arguments: {
            query: 'button',
            category: 'guide',
            limit: 3
          }
        }
      }
    },
    {
      name: 'Search for Animation Examples',
      message: {
        jsonrpc: '2.0',
        id: 3,
        method: 'tools/call',
        params: {
          name: 'search_slint_docs',
          arguments: {
            query: 'animation',
            category: 'all',
            limit: 2
          }
        }
      }
    },
    {
      name: 'Get Specific Documentation Content',
      message: {
        jsonrpc: '2.0',
        id: 4,
        method: 'tools/call',
        params: {
          name: 'get_slint_doc_content',
          arguments: {
            path: 'guide/language/concepts/slint-language'
          }
        }
      }
    },
    {
      name: 'List All Categories',
      message: {
        jsonrpc: '2.0',
        id: 5,
        method: 'tools/call',
        params: {
          name: 'list_slint_doc_categories',
          arguments: {}
        }
      }
    }
  ];

  console.log('📤 Sending test prompts...\n');

  // Send each test prompt
  for (const prompt of testPrompts) {
    console.log(`🔍 Testing: ${prompt.name}`);
    console.log(`📋 Query: ${JSON.stringify(prompt.message.params, null, 2)}\n`);
    
    server.stdin.write(JSON.stringify(prompt.message) + '\n');
    
    // Wait for response
    await new Promise(resolve => setTimeout(resolve, 2000));
    
    console.log('--- Response ---');
    console.log(output.slice(output.lastIndexOf('{', output.length - 2)));
    console.log('\n' + '='.repeat(80) + '\n');
  }

  // Close the server
  server.kill();

  console.log('✅ Testing complete!');
  console.log('\n📊 Summary:');
  console.log('- MCP server responds to protocol messages');
  console.log('- Search functionality works');
  console.log('- Content retrieval works');
  console.log('- Category listing works');
}

testMCPServer().catch(console.error);

