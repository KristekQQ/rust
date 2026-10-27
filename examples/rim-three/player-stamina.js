export const MAX_STAMINA=4;
export const REGEN_DELAY=2;
export const REGEN_RATE=1;

// Real-time clock, independent of bullet time. Partial units refill visibly.
export class PlayerStamina {
  constructor(){this.reset()}
  reset(){this.value=MAX_STAMINA;this.delay=0;this.spent=0}
  canSpend(){return this.value>=1-1e-9}
  spend(){if(!this.canSpend())return false;this.value=Math.max(0,this.value-1);this.delay=REGEN_DELAY;this.spent++;return true}
  update(dt){
    if(!Number.isFinite(dt)||dt<=0)return;
    const recovery=Math.max(0,dt-this.delay);
    this.delay=Math.max(0,this.delay-dt);
    this.value=Math.min(MAX_STAMINA,this.value+recovery*REGEN_RATE);
  }
  describe(){return {value:this.value,max:MAX_STAMINA,available:Math.floor(this.value+1e-9),regenDelay:this.delay,spent:this.spent}}
}
