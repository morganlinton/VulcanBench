// Hidden pass-to-pass guards: ordinary node-adapter requests unchanged.
import { test } from 'node:test';
import assert from 'node:assert';
import http from 'node:http';
import axios from './index.js';

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

test('vb basic get works', async () => {
  await withServer(
    (req, res) => {
      res.setHeader('content-type', 'application/json');
      res.end(JSON.stringify({ ok: true, path: req.url }));
    },
    async (base) => {
      const res = await axios.get(base + '/hello');
      assert.equal(res.status, 200);
      assert.equal(res.data.ok, true);
      assert.equal(res.data.path, '/hello');
    }
  );
});

test('vb post sends a json body', async () => {
  await withServer(
    (req, res) => {
      let buf = '';
      req.on('data', (c) => (buf += c));
      req.on('end', () => res.end(buf));
    },
    async (base) => {
      const res = await axios.post(base + '/', { n: 7 });
      const body = typeof res.data === 'string' ? JSON.parse(res.data) : res.data;
      assert.equal(body.n, 7);
    }
  );
});

test('vb existing method header slots still present', () => {
  for (const method of ['get', 'post', 'put', 'patch', 'delete', 'head', 'common']) {
    assert.ok(axios.defaults.headers[method] !== undefined, method);
  }
});

test('vb http error status rejects with AxiosError', async () => {
  await withServer(
    (req, res) => {
      res.statusCode = 500;
      res.end('nope');
    },
    async (base) => {
      await assert.rejects(axios.get(base + '/'), (err) => {
        assert.equal(err.isAxiosError, true);
        assert.equal(err.response.status, 500);
        return true;
      });
    }
  );
});
