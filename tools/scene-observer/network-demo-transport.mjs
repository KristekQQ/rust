/** Delivery timing only. Scene poses come from the Rust recording. */
export class SimulatedDelivery {
 constructor({delay=150,jitter=0,random=Math.random}={}){this.delay=delay;this.jitter=jitter;this.random=random;this.queue=[];this.lastDue=-Infinity;this.stallUntil=0;}
 enqueue(frame,now,sampledAt){
  if(this.queue.length>=128)throw Error('Test delivery queue exceeded128 states');
  const due=Math.max(now+Math.max(0,this.delay+(this.random()*2-1)*this.jitter),this.lastDue+0.01);
  this.queue.push({frame,due,sampledAt});this.lastDue=due;
 }
 receive(now){
  if(now<this.stallUntil)return null;
  let count=0;while(count<this.queue.length&&this.queue[count].due<=now)count++;
  if(!count)return null;
  const value=this.queue[count-1];this.queue.splice(0,count);return {...value,coalesced:count-1};
 }
 stall(now,ms=2000){this.stallUntil=now+ms;}
}
