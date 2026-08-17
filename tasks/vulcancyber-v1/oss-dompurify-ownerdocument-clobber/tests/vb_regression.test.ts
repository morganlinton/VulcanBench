import { test } from 'node:test'
import assert from 'node:assert'
import { freshDOMPurify } from './vb_setup.ts'

// pass_to_pass guards for PR #1560: the ordinary, un-clobbered behaviour the
// ownerDocument hardening must not disturb. These pass at the base commit and
// after the fix alike — the change only alters how the document is
// dereferenced, never the sanitize result for a normal tree.

test('vb IN_PLACE without any clobber strips an on-event handler as usual', () => {
  const { DOMPurify, document } = freshDOMPurify()
  const root = document.createElement('div')
  const img = document.createElement('img')
  img.setAttribute('onerror', 'alert(1)')
  root.appendChild(img)
  document.body.appendChild(root)

  DOMPurify.sanitize(root, { IN_PLACE: true })

  assert.strictEqual(
    img.getAttribute('onerror'),
    null,
    'unclobbered IN_PLACE sanitize still removes the handler'
  )
})

test('vb string sanitize keeps benign markup and drops dangerous attributes', () => {
  const { DOMPurify } = freshDOMPurify()
  const out = DOMPurify.sanitize(
    '<a href="https://example.com">ok</a><img src="x" onerror="alert(1)">'
  )

  assert.ok(
    out.indexOf('<a href="https://example.com">ok</a>') > -1,
    'benign anchor preserved'
  )
  assert.ok(!/onerror/i.test(out), 'dangerous on* attribute removed')
  assert.ok(!/<script/i.test(out), 'no script element introduced')
})
