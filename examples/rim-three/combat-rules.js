// Shared encounter contract with Rust rim_game: bounded pools and readable damage.
export const WAVES=[['drone-orbiter',6],['core-tank',2],['blade-wraith',2],['drone-orbiter',8],['core-tank',3],['blade-wraith',3]];
export const POOL_COUNTS={'drone-orbiter':8,'core-tank':3,'blade-wraith':3};
export const MAX_HP={'drone-orbiter':100,'core-tank':200,'blade-wraith':150};
export const SWORD_DAMAGE=50,REFLECT_DAMAGE=100;
export function healthFor(kind){return {hp:MAX_HP[kind],maxHp:MAX_HP[kind],lastStrikes:[-1,-1],contacts:[false,false],cooldowns:[0,0]};}
export function tickHealth(health,dt){for(let h=0;h<2;h++)health.cooldowns[h]=Math.max(0,health.cooldowns[h]-dt);}
export function bladeDamageAllowed(health,hand,contact,eligible,strike=null){
 if(!contact){health.contacts[hand]=false;return false;}
 if(health.contacts[hand]||!eligible||health.hp<=0||health.cooldowns[hand]>0||(strike!==null&&health.lastStrikes[hand]===strike))return false;
 health.contacts[hand]=true;health.cooldowns[hand]=.28;if(strike!==null)health.lastStrikes[hand]=strike;return true;
}
export function hurt(health,amount){if(health.hp<=0||!Number.isFinite(amount)||amount<=0)return false;health.hp=Math.max(0,health.hp-amount);return true;}
