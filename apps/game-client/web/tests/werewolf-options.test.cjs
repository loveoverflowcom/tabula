const test=require('node:test'),assert=require('node:assert/strict');
const launch=require('../launch-options.js');
test('isolated-seat simulator config preserves bounded public preferences',()=>{
  for(const seats of [6,10,12,20]){
    const cfg=launch.parse(`game=werewolf&mode=simulator&seats=${seats}&theme=hc-dark&locale=vi`);
    assert.equal(cfg.seats,seats);assert.equal(cfg.mode,'simulator');
    assert.equal(launch.parse(launch.query(cfg)).game,'werewolf');
    assert.equal(launch.argumentsFor(launch.resolve(cfg,()=>({matches:false}))),`--seats\n${seats}\n--theme\nhc-dark`);
  }
});
test('simulator rejects expansion, credentials, repeated settings and online modes',()=>{
  for(const suffix of ['seats=5','seats=21','seats=12&seats=12','mode=online','source=tabula','return_to=https://example.com','seed=0','roles=witch','token=secret','clock=untimed','theme=unknown']){
    assert.throws(()=>launch.parse(`game=werewolf&${suffix}`));
  }
});
test('classic host configuration remains unchanged',()=>{
  const cfg=launch.parse('game=chess&mode=hot-seat&clock=untimed');
  assert.equal(cfg.clock,'untimed');assert.match(launch.argumentsFor(launch.resolve(cfg,()=>({matches:false}))),/^--game\nchess\n/);
});
