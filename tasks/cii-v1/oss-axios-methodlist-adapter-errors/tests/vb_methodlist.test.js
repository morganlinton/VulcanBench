// Hidden fail-to-pass tests: the shared method list must cover every method
// axios exposes, and HTTP adapter option errors must surface as AxiosError
// (never bare TypeError). Node http adapter + loopback servers only.
import { test } from 'node:test';
import assert from 'node:assert';
import http from 'node:http';
import axios from './index.js';

const AxiosError = axios.AxiosError;

const withServer = async (handler, fn) => {
  const server = http.createServer(handler);
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  const port = server.address().port;
  try {
    return await fn(`http://127.0.0.1:${port}`);
  } finally {
    server.close();
  }
};

test('vb defaults expose per-method header slots for the full method list', () => {
  for (const method of ['options', 'purge', 'link', 'unlink', 'query', 'get', 'post']) {
    assert.ok(
      axios.defaults.headers[method] && typeof axios.defaults.headers[method] === 'object',
      `defaults.headers.${method} must exist`
    );
  }
});

test('vb method-scoped default headers apply to OPTIONS requests', async () => {
  await withServer(
    (req, res) => {
      res.end(JSON.stringify({ method: req.method, marker: req.headers['x-vb-marker'] || null }));
    },
    async (base) => {
      const instance = axios.create();
      instance.defaults.headers.options['X-VB-Marker'] = 'yes';
      const res = await instance.request({ url: base + '/', method: 'options' });
      const body = typeof res.data === 'string' ? JSON.parse(res.data) : res.data;
      assert.equal(body.method, 'OPTIONS');
      assert.equal(body.marker, 'yes');
    }
  );
});

test('vb invalid httpVersion rejects with AxiosError bad option value', async () => {
  await assert.rejects(
    axios.get('http://127.0.0.1:9/', { httpVersion: 'not-a-number' }),
    (err) => {
      assert.equal(err.isAxiosError, true, 'must be an AxiosError');
      assert.equal(err.code, AxiosError.ERR_BAD_OPTION_VALUE);
      return true;
    }
  );
});

test('vb bad lookup address rejects with AxiosError instead of crashing', async () => {
  await assert.rejects(
    axios.get('http://vb-internal.test/', {
      lookup: (host, opt, cb) => cb(null, 12345, 4),
    }),
    (err) => {
      assert.equal(err.isAxiosError, true, 'must be an AxiosError');
      assert.equal(err.code, AxiosError.ERR_BAD_OPTION_VALUE);
      return true;
    }
  );
});
