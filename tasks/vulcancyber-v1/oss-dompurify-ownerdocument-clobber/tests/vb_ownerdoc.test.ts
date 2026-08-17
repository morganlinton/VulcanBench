import { test } from 'node:test'
import assert from 'node:assert'
import { freshDOMPurify } from './vb_setup.ts'

// See PR #1560: HTMLFormElement's [LegacyOverrideBuiltIns] lets a child named
// "ownerDocument" shadow Node.prototype.ownerDocument, so a direct read returns
// that element. jsdom does not implement that named-getter override, so — as the
// upstream suite does — we reproduce the identical shape with an own accessor.
// At base, _createNodeIterator reads the clobbered property and calls
// createNodeIterator.call(<the element>, ...), which throws before the IN_PLACE
// walk's fail-closed barrier, leaving armed descendants un-neutralized. The
// invariant holds whether sanitize returns or fails closed by throwing.

function clobber(el: any, document: any) {
  const fake = document.createElement('input')
  Object.defineProperty(el, 'ownerDocument', {
    get() {
      return fake
    },
    configurable: true,
  })
  return fake
}

test('vb clobbered ownerDocument still strips a direct on-event handler', () => {
  const { DOMPurify, document } = freshDOMPurify()
  const root = document.createElement('form')
  const img = document.createElement('img')
  img.setAttribute('onerror', 'alert(1)') // no src: assert the attr, not a load
  root.appendChild(img)
  document.body.appendChild(root)

  const fake = clobber(root, document)
  assert.strictEqual(root.ownerDocument, fake, 'precondition: read is clobbered')

  try {
    DOMPurify.sanitize(root, { IN_PLACE: true })
  } catch {
    /* a fail-closed throw is acceptable, as long as the scrub ran */
  }

  assert.strictEqual(
    img.getAttribute('onerror'),
    null,
    'armed on* handler stripped despite the ownerDocument clobber'
  )
})

test('vb clobbered ownerDocument still neutralizes a deeply nested handler', () => {
  const { DOMPurify, document } = freshDOMPurify()
  const root = document.createElement('form')
  const section = document.createElement('section')
  const link = document.createElement('a')
  link.setAttribute('href', '#')
  link.setAttribute('onclick', 'alert(1)')
  section.appendChild(link)
  root.appendChild(section)
  document.body.appendChild(root)

  clobber(root, document)

  try {
    DOMPurify.sanitize(root, { IN_PLACE: true })
  } catch {
    /* fail-closed throw acceptable */
  }

  assert.strictEqual(
    link.getAttribute('onclick'),
    null,
    'nested on* handler stripped despite the ownerDocument clobber'
  )
})

test('vb clobbered ownerDocument leaves no on-event handler in the serialized tree', () => {
  const { DOMPurify, document } = freshDOMPurify()
  const root = document.createElement('form')
  root.innerHTML =
    '<div><img src="x" onerror="alert(1)"><span onmouseover="alert(2)">x</span></div>'
  document.body.appendChild(root)

  clobber(root, document)

  try {
    DOMPurify.sanitize(root, { IN_PLACE: true })
  } catch {
    /* fail-closed throw acceptable */
  }

  assert.ok(
    !/on(error|mouseover)\s*=/i.test(root.outerHTML),
    'no on* handler survives in the in-place tree after the clobber'
  )
})
