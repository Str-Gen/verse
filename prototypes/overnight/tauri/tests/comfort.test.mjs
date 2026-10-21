import test from 'node:test';
import assert from 'node:assert/strict';
import {Comfort} from '../web/comfort.js';
test('preparation cancellation, ramp, brief resume and fresh long pause', () => {
 const comfort = new Comfort();
 assert.equal(comfort.start(0),2000);
 comfort.pause(500); assert.equal(comfort.start(600),2000);
 assert.equal(comfort.fraction(2600),.5);
 assert.equal(comfort.fraction(17600),.75);
 assert.equal(comfort.fraction(32600),1);
 comfort.pause(33000); assert.equal(comfort.start(34000),0); assert.equal(comfort.fraction(34000),1);
 comfort.pause(35000); assert.equal(comfort.start(66000),2000);
 comfort.pause(66500); assert.equal(comfort.start(67000),2000);
});
