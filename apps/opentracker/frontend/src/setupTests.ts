// Polyfill for jsdom environment
import { TextEncoder, TextDecoder } from 'util';
import axios from 'axios';

global.TextEncoder = TextEncoder;
// @ts-ignore
global.TextDecoder = TextDecoder;

// Configure axios for tests
axios.defaults.baseURL = 'http://localhost:3025/api';
