export interface AudioEvent {id:number;at:number;clip:string;position:[number,number,number]|null;gain:number;pitch:number;priority:number}
export interface AudioPolicy {maxVoices:number;master:number;groups:{prefix:string;limit:number}[]}
export interface AudioSnapshot {epoch:number;clock:number;latest:number;events:AudioEvent[];policy?:AudioPolicy}
export interface AudioOptions {position?:[number,number,number]|null;gain?:number;pitch?:number;priority?:number;delay?:number;loop?:boolean;bus?:'effects'|'ui'|'music'}
export class AudioClip {constructor(id:string,buffer:AudioBuffer,options?:{gain?:number;spatial?:boolean});id:string;buffer:AudioBuffer}
export class AudioListener {constructor(context:BaseAudioContext);setPose(position:number[],forward?:number[],up?:number[]):void;setMatrix(matrix:ArrayLike<number>):void}
export class AudioSource {setPosition(position:number[]):void;stop():void;dispose():void}
export class AudioMixer {
 constructor(options?:{context?:AudioContext|OfflineAudioContext;maxVoices?:number;master?:number});
 readonly context:AudioContext|OfflineAudioContext;readonly listener:AudioListener;readonly clips:Map<string,AudioClip>;readonly voices:Set<AudioSource>;
 readonly stats:{played:number;expired:number;limited:number;gaps:number};
 load(manifestUrl:URL|string):Promise<void>;resume():Promise<void>;setVolume(value:number,bus?:'effects'|'ui'|'music'|null):void;
 play(id:string,options?:AudioOptions):AudioSource|null;
 ingest(snapshot:AudioSnapshot,options?:{suppressInitial?:boolean;delay?:number;maxAge?:number;transportAge?:number}):void;
 setOutputEnabled(enabled:boolean):void;reset():void;stopAll():void;suspend():Promise<void>;dispose():Promise<void>;
}
