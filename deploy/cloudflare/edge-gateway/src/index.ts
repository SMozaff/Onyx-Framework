import { argon2id, argon2Verify } from "hash-wasm";
import { adminCreateUser, adminSetUserBanned, getSupabaseUser, passwordSignIn, verifySupabaseJwt } from "./supabase";

type Json = Record<string, unknown>;

interface Env {
  DB: D1Database;
  ONYX_JWT_SECRET?: string;
  ONYX_CORS_ORIGINS?: string;
  ONYX_ALLFATHER_EMAIL?: string;
  SUPABASE_URL?: string;
  SUPABASE_JWKS_URL?: string;
  SUPABASE_PUBLISHABLE_KEY?: string;
  SUPABASE_SECRET_KEY?: string;
  HF_S3_ENDPOINT?: string;
  HF_S3_BUCKET?: string;
  HF_S3_ACCESS_KEY_ID?: string;
  HF_S3_SECRET_ACCESS_KEY?: string;
}

interface Session {
  sub: string;
  username: string;
  organization_id: string;
  is_admin: boolean;
  class: string | null;
  role: string;
  client_type: string;
  exp: number;
  jti: string;
  typ: "access" | "refresh";
}

const JSON_HEADERS = { "content-type": "application/json; charset=utf-8", "cache-control": "no-store" };
const ALLFATHER_EMAIL = "so.muzaff@gmail.com";
const OBSERVER_QUERIES: Record<string, string> = {
  "mission.list":"mission", "mission.detail":"mission", "task.list":"task", "task.detail":"task",
  "timeline.list":"*", "notification.list":"notification", "approval.list":"approval", "report.detail":"report",
  "policy.list":"policy", "policy.detail":"policy", "legal_hold.list":"legal_hold", "legal_hold.detail":"legal_hold",
  "todo_list.list":"todo_list", "todo_list.detail":"todo_list", "target_list.list":"target_list", "target_list.detail":"target_list",
  "staff_loan.list":"staff_loan", "staff_loan.detail":"staff_loan",
};

function json(body: unknown, status = 200, origin?: string | null): Response {
  const headers = new Headers(JSON_HEADERS);
  headers.set("access-control-allow-origin", origin ?? "*");
  headers.set("access-control-allow-credentials", "true");
  return new Response(JSON.stringify(body), { status, headers });
}
function now() { return Math.floor(Date.now() / 1000); }
function uuid() { return crypto.randomUUID(); }
function b64u(value: Uint8Array | string): string {
  const bytes = typeof value === "string" ? new TextEncoder().encode(value) : value;
  let binary = ""; for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary).replace(/\+/g,"-").replace(/\//g,"_").replace(/=+$/g,"");
}
function fromB64u(value: string): Uint8Array {
  const normalized = value.replace(/-/g,"+").replace(/_/g,"/") + "=".repeat((4-(value.length%4))%4);
  const binary = atob(normalized); return Uint8Array.from(binary,(c)=>c.charCodeAt(0));
}
function decodeJsonB64u(value: string): Json {
  return JSON.parse(new TextDecoder().decode(fromB64u(value))) as Json;
}
async function hmac(secret:string,data:string):Promise<Uint8Array>{
  const key=await crypto.subtle.importKey("raw",new TextEncoder().encode(secret),{name:"HMAC",hash:"SHA-256"},false,["sign"]);
  return new Uint8Array(await crypto.subtle.sign("HMAC",key,new TextEncoder().encode(data)));
}
async function sha256(value:string):Promise<string>{
  const digest=await crypto.subtle.digest("SHA-256",new TextEncoder().encode(value));
  return Array.from(new Uint8Array(digest),(b)=>b.toString(16).padStart(2,"0")).join("");
}
async function jwtSign(env:Env,session:Omit<Session,"jti">):Promise<string>{
  if(!env.ONYX_JWT_SECRET) throw new Error("ONYX_JWT_SECRET is not configured");
  const header=b64u(JSON.stringify({alg:"HS256",typ:"JWT"})); const payload=b64u(JSON.stringify({...session,jti:uuid()}));
  const input=`${header}.${payload}`; return `${input}.${b64u(await hmac(env.ONYX_JWT_SECRET,input))}`;
}
async function jwtVerify(env:Env,token:string):Promise<Session>{
  if(!env.ONYX_JWT_SECRET) throw new Error("ONYX_JWT_SECRET is not configured");
  const parts=token.split("."); if(parts.length!==3) throw new Error("invalid token");
  if(b64u(await hmac(env.ONYX_JWT_SECRET,`${parts[0]}.${parts[1]}`))!==parts[2]) throw new Error("invalid token");
  const session=JSON.parse(new TextDecoder().decode(fromB64u(parts[1]))) as Session;
  if(!session.sub||!session.organization_id||!session.jti||session.exp<=now()) throw new Error("expired token");
  const revoked=await env.DB.prepare("SELECT 1 FROM token_revocations WHERE token_hash=? LIMIT 1").bind(await sha256(token)).first();
  if(revoked) throw new Error("revoked token"); return session;
}
function authorization(request:Request){const value=request.headers.get("authorization");return value?value.replace(/^Bearer\s+/i,""):null;}
async function requireSession(request:Request,env:Env){const token=authorization(request);if(!token)throw new Error("AUTH_REQUIRED");return jwtVerify(env,token);}
function corsOrigin(request:Request,env:Env){const origin=request.headers.get("origin");const configured=env.ONYX_CORS_ORIGINS?.split(",").map(x=>x.trim()).filter(Boolean)??[];if(!origin)return configured[0]??null;if(configured.length===0||configured.includes("*"))return origin;return configured.includes(origin)?origin:null;}

async function verifyPassword(password:string,encoded:string):Promise<boolean>{
  if(!encoded.startsWith("$argon2id$")) return false;
  try { return await argon2Verify({password,hash:encoded}); } catch { return false; }
}
async function hashPassword(password:string):Promise<string>{
  const salt=crypto.getRandomValues(new Uint8Array(16));
  return String(await argon2id({password,salt,parallelism:1,iterations:3,memorySize:65536,hashLength:32,outputType:"encoded"}));
}
function clientType(payload:Json){return typeof payload.client_type==="string"?payload.client_type:"web";}
function roleForUser(user:Record<string,unknown>):string{
  const explicit=typeof user.role==="string"?user.role:"";
  if(explicit) return explicit;
  if(String(user.username).toLowerCase()==="allfather") return "ALL_FATHER";
  return Boolean(user.is_admin) ? "ORGANIZATION_ADMIN" : "STAFF";
}
async function issuePair(env:Env,user:Record<string,unknown>,type:string){
  const role=roleForUser(user);
  const base={sub:String(user.id),username:String(user.username),organization_id:String(user.organization_id),is_admin:Boolean(user.is_admin),class:(user.class as string|null)??null,role,client_type:type};
  const access=await jwtSign(env,{...base,exp:now()+3600,typ:"access"}); const refresh=await jwtSign(env,{...base,exp:now()+7*86400,typ:"refresh"});
  return {access_token:access,refresh_token:refresh,expires_in:3600,user:{id:user.id,username:user.username,email:user.email??null,organization_id:user.organization_id,is_admin:Boolean(user.is_admin),class:user.class??null,role}};
}
async function authLogin(request:Request,env:Env){
  const body=await request.json().catch(()=>null) as Json|null;
  const identifier=typeof body?.username==="string"?body.username.trim():"";
  const password=typeof body?.password==="string"?body.password:"";
  if(!identifier||!password)return json({error:"INVALID_CREDENTIALS",category:"AUTHORITY",retryability:"NON_RETRYABLE"},401);
  const user=await env.DB.prepare("SELECT id,username,email,supabase_user_id,organization_id,is_admin,is_active,class,role FROM users WHERE LOWER(username)=LOWER(?) OR LOWER(email)=LOWER(?) LIMIT 1").bind(identifier,identifier).first<Record<string,unknown>>();
  if(!user?.is_active||roleForUser(user)==="ALL_FATHER"||typeof user?.email!=="string")return json({error:"INVALID_CREDENTIALS",category:"AUTHORITY",retryability:"NON_RETRYABLE"},401);
  let supabaseSession:{access_token:string;refresh_token:string;user:Json};
  try{supabaseSession=await passwordSignIn(env,String(user.email),password);}catch{return json({error:"INVALID_CREDENTIALS",category:"AUTHORITY",retryability:"NON_RETRYABLE"},401);}
  const supabaseUserId=typeof supabaseSession.user.id==="string"?supabaseSession.user.id:"";
  if(!supabaseUserId)return json({error:"AUTH_ACCOUNT_INVALID",category:"AUTHORITY",retryability:"NON_RETRYABLE"},401);
  if(String(user.supabase_user_id||"")!==supabaseUserId){
    await env.DB.prepare("UPDATE users SET supabase_user_id=?,updated_at=? WHERE id=?").bind(supabaseUserId,Date.now(),String(user.id)).run();
    user.supabase_user_id=supabaseUserId;
  }
  return json(await issuePair(env,user,clientType(body)));
}
async function authRefresh(request:Request,env:Env){
  const body=await request.json().catch(()=>null) as Json|null; const token=typeof body?.refresh_token==="string"?body.refresh_token:"";
  try{const claims=await jwtVerify(env,token);if(claims.typ!=="refresh")throw new Error("wrong token type");const user=await env.DB.prepare("SELECT id,username,email,organization_id,is_admin,is_active,class,role FROM users WHERE id=? LIMIT 1").bind(claims.sub).first<Record<string,unknown>>();if(!user?.is_active)throw new Error("inactive");await env.DB.prepare("INSERT OR IGNORE INTO token_revocations(token_hash,revoked_at) VALUES(?,?)").bind(await sha256(token),now()).run();return json(await issuePair(env,user,claims.client_type));}catch{return json({error:"INVALID_REFRESH_TOKEN",category:"AUTHORITY",retryability:"NON_RETRYABLE"},401);}
}
async function authLogout(request:Request,env:Env){const token=authorization(request);const body=await request.json().catch(()=>({})) as Json;for(const value of [token,typeof body.refresh_token==="string"?body.refresh_token:null].filter((x):x is string=>Boolean(x)))await env.DB.prepare("INSERT OR IGNORE INTO token_revocations(token_hash,revoked_at) VALUES(?,?)").bind(await sha256(value),now()).run();return json({success:true});}

async function authSupabaseAllFather(request:Request,env:Env){
  const token=authorization(request);
  if(!token)return json({error:"AUTH_REQUIRED",category:"AUTHORITY",retryability:"NON_RETRYABLE"},401);
  try{
    const claims=await verifySupabaseJwt(env,token);
    const identity=await getSupabaseUser(env,token);
    const email=(identity.email||"").trim().toLowerCase();
    const expected=(env.ONYX_ALLFATHER_EMAIL||ALLFATHER_EMAIL).trim().toLowerCase();
    // The signed JWT alone does not prove that the current Supabase user
    // still has a confirmed email. Resolve the bearer token through Supabase
    // Auth and bind the returned user ID to the JWT subject before granting
    // the All-Father role.
    if(identity.id!==claims.sub||!identity.email_confirmed_at||!email||email!==expected)
      return json({error:"ALLFATHER_REQUIRED",category:"AUTHORITY",retryability:"NON_RETRYABLE"},403);
    const user=await env.DB.prepare("SELECT id,username,email,supabase_user_id,organization_id,is_admin,is_active,class,role FROM users WHERE LOWER(username)=LOWER('allfather') LIMIT 1").first<Record<string,unknown>>();
    if(!user?.is_active||roleForUser(user)!=="ALL_FATHER"||!user.is_admin)return json({error:"ALLFATHER_NOT_PROVISIONED",category:"AUTHORITY",retryability:"NON_RETRYABLE"},403);
    if(String(user.supabase_user_id||"")!==String(claims.sub)){
      await env.DB.prepare("UPDATE users SET supabase_user_id=?,updated_at=? WHERE id=?").bind(String(claims.sub),Date.now(),String(user.id)).run();
      user.supabase_user_id=String(claims.sub);
    }
    return json(await issuePair(env,user,"allfather"));
  }catch(error){
    const message=error instanceof Error?error.message:"invalid token";
    return json({error:message,category:"AUTHORITY",retryability:"NON_RETRYABLE"},401);
  }
}
function requireRole(session:Session,...roles:string[]){if(!roles.includes(session.role))throw new Error("FORBIDDEN_ROLE");}
function requiredString(body:Json,key:string):string{const value=body[key];if(typeof value!=="string"||!value.trim())throw new Error(`INVALID_${key.toUpperCase()}`);return value.trim();}
function optionalEmail(body:Json):string|null{const value=typeof body.email==="string"?body.email.trim().toLowerCase():null;return value||null;}
async function createOrganization(request:Request,env:Env,session:Session){
  requireRole(session,"ALL_FATHER");const body=await request.json().catch(()=>null) as Json|null;if(!body)throw new Error("INVALID_BODY");
  const name=requiredString(body,"name");const id=typeof body.id==="string"&&body.id.trim()?body.id.trim():uuid();const timestamp=Date.now();
  try{await env.DB.prepare("INSERT INTO organizations(id,name,is_active,created_at,updated_at) VALUES(?,?,1,?,?)").bind(id,name,timestamp,timestamp).run();await audit(env,session,"organization.create",id,{name});return json({id,name,is_active:true});}catch{return json({error:"ORGANIZATION_EXISTS",category:"DOMAIN",retryability:"NON_RETRYABLE"},409);}
}
async function createUser(request:Request,env:Env,session:Session,targetRole:"ORGANIZATION_ADMIN"|"STAFF"){
  requireRole(session,targetRole==="ORGANIZATION_ADMIN"?"ALL_FATHER":"ORGANIZATION_ADMIN");
  const body=await request.json().catch(()=>null) as Json|null;if(!body)throw new Error("INVALID_BODY");
  const username=requiredString(body,"username"),password=requiredString(body,"password"),email=optionalEmail(body);
  if(password.length<8)throw new Error("PASSWORD_TOO_SHORT");
  if(!email)throw new Error("EMAIL_REQUIRED");
  const organizationId=targetRole==="STAFF"?session.organization_id:requiredString(body,"organization_id");
  const org=await env.DB.prepare("SELECT id FROM organizations WHERE id=? AND is_active=1 LIMIT 1").bind(organizationId).first();
  if(!org)throw new Error("ORGANIZATION_NOT_FOUND");
  const existing=await env.DB.prepare("SELECT id FROM users WHERE LOWER(username)=LOWER(?) OR (? IS NOT NULL AND LOWER(email)=LOWER(?)) LIMIT 1").bind(username,email,email).first();
  if(existing)return json({error:"USER_EXISTS",category:"DOMAIN",retryability:"NON_RETRYABLE"},409);
  let supabaseUserId:string;
  try{supabaseUserId=await adminCreateUser(env,email,password,{onyx_username:username,onyx_role:targetRole,onyx_organization_id:organizationId});}
  catch(error){return json({error:error instanceof Error?error.message:"SUPABASE_USER_CREATE_FAILED",category:"AUTHORITY",retryability:"NON_RETRYABLE"},502);}
  const id=uuid(),timestamp=Date.now(),isAdmin=targetRole==="ORGANIZATION_ADMIN"?1:0,parent=targetRole==="STAFF"?session.sub:"__allfather__";
  const placeholderHash="$argon2id$v=19$m=65536,t=3,p=4$AAAAAAAAAAAAAAAAAAAAAA$AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
  try{await env.DB.prepare("INSERT INTO users(id,username,email,supabase_user_id,organization_id,password_hash,is_admin,is_active,class,role,parent_user_id,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?)").bind(id,username,email,supabaseUserId,organizationId,placeholderHash,isAdmin,1,targetRole,targetRole,parent,timestamp,timestamp).run();}
  catch(error){try{await adminSetUserBanned(env,supabaseUserId,true);}catch{} throw error;}
  await audit(env,session,targetRole==="STAFF"?"staff.create":"admin.create",id,{username,email,organization_id:organizationId,supabase_user_id:supabaseUserId});
  return json({id,username,email,organization_id:organizationId,is_admin:Boolean(isAdmin),is_active:true,role:targetRole},201);
}
async function deactivateUser(request:Request,env:Env,session:Session,userId:string){
  const target=await env.DB.prepare("SELECT id,username,email,supabase_user_id,organization_id,is_admin,is_active,role FROM users WHERE id=? LIMIT 1").bind(userId).first<Record<string,unknown>>();
  if(!target)return json({error:"USER_NOT_FOUND",category:"DOMAIN",retryability:"NON_RETRYABLE"},404);
  const role=roleForUser(target);
  if(session.role==="ALL_FATHER"&&role!=="ORGANIZATION_ADMIN")return json({error:"FORBIDDEN_ROLE",category:"AUTHORITY",retryability:"NON_RETRYABLE"},403);
  if(session.role==="ORGANIZATION_ADMIN"&&(role!=="STAFF"||String(target.organization_id)!==session.organization_id))return json({error:"FORBIDDEN_ROLE",category:"AUTHORITY",retryability:"NON_RETRYABLE"},403);
  if(role==="ALL_FATHER")return json({error:"FORBIDDEN_ROLE",category:"AUTHORITY",retryability:"NON_RETRYABLE"},403);
  if(target.supabase_user_id){try{await adminSetUserBanned(env,String(target.supabase_user_id),true);}catch(error){return json({error:error instanceof Error?error.message:"SUPABASE_USER_DEACTIVATION_FAILED",category:"AUTHORITY",retryability:"RETRYABLE"},502);}}
  await env.DB.prepare("UPDATE users SET is_active=0,updated_at=? WHERE id=?").bind(Date.now(),userId).run();
  await audit(env,session,"user.deactivate",userId,{username:target.username,role});
  return json({success:true,id:userId,is_active:false});
}
async function audit(env:Env,session:Session,action:string,targetId:string,details:Json){await env.DB.prepare("INSERT INTO audit_log(organization_id,user_id,action,correlation_id,details,created_at) VALUES(?,?,?,?,?,?)").bind(session.organization_id,session.sub,action,uuid(),JSON.stringify({target:targetId,...details}),Date.now()).run();}

function queryEnvelope(raw:string):Json|null{try{return JSON.parse(new TextDecoder().decode(fromB64u(raw))) as Json;}catch{return null;}}
function safeJson(value:string):unknown{try{return JSON.parse(value);}catch{return value;}}
async function queryRoute(request:Request,env:Env,session:Session){
  const raw=new URL(request.url).searchParams.get("envelope");const envelope=raw?queryEnvelope(raw):null;
  if(!envelope||typeof envelope.query_id!=="string"||typeof envelope.query_type!=="string"||typeof envelope.organization_id!=="string")return json({error:"INVALID_QUERY_ENVELOPE",category:"DOMAIN",retryability:"NON_RETRYABLE"},400);
  if(envelope.organization_id!==session.organization_id)return json({error:"TENANT_MISMATCH",category:"AUTHORITY",retryability:"NON_RETRYABLE"},403);
  const type=String(envelope.query_type);
  if(type==="dashboard.summary"){const row=await env.DB.prepare("SELECT aggregate_type,COUNT(*) AS count FROM aggregates WHERE organization_id=? GROUP BY aggregate_type").bind(session.organization_id).all();return json({query_id:envelope.query_id,data:row.results??[],has_more:false,next_cursor:null,total_count:row.results?.length??0,freshness:"d1"});}
  const aggregateType=OBSERVER_QUERIES[type];if(!aggregateType)return json({query_id:envelope.query_id,data:[],has_more:false,next_cursor:null,total_count:0,freshness:"d1"});
  const filters=(envelope.filters??{}) as Json;const targetId=typeof filters.target_id==="string"?filters.target_id:typeof envelope.target_id==="string"?envelope.target_id:null;
  let rows;
  if(aggregateType==="*")rows=await env.DB.prepare("SELECT id,aggregate_type,version,lifecycle_epoch,authority_epoch,state,updated_at FROM aggregates WHERE organization_id=? ORDER BY updated_at DESC LIMIT 100").bind(session.organization_id).all<Record<string,unknown>>();
  else if(targetId)rows=await env.DB.prepare("SELECT id,aggregate_type,version,lifecycle_epoch,authority_epoch,state,updated_at FROM aggregates WHERE organization_id=? AND aggregate_type=? AND id=? LIMIT 1").bind(session.organization_id,aggregateType,targetId).all<Record<string,unknown>>();
  else rows=await env.DB.prepare("SELECT id,aggregate_type,version,lifecycle_epoch,authority_epoch,state,updated_at FROM aggregates WHERE organization_id=? AND aggregate_type=? ORDER BY updated_at DESC LIMIT 100").bind(session.organization_id,aggregateType).all<Record<string,unknown>>();
  const data=(rows.results??[]).map(row=>({...row,state:typeof row.state==="string"?safeJson(row.state):row.state}));return json({query_id:envelope.query_id,data,has_more:false,next_cursor:null,total_count:data.length,freshness:"d1"});
}
async function commandRoute(request:Request,env:Env,session:Session){
  if(!session.is_admin&&session.client_type==="mobile_observer")return json({error:"CAPABILITY_DENIED",category:"AUTHORITY",retryability:"NON_RETRYABLE"},403);
  const envelope=await request.json().catch(()=>null) as Json|null;if(!envelope||typeof envelope.command_id!=="string"||typeof envelope.operation_id!=="string"||typeof envelope.command_type!=="string")return json({error:"INVALID_COMMAND_ENVELOPE",category:"DOMAIN",retryability:"NON_RETRYABLE"},400);
  const target=envelope.target as Json;if(target?.organization_id!==session.organization_id)return json({error:"TENANT_MISMATCH",category:"AUTHORITY",retryability:"NON_RETRYABLE"},403);
  const prior=await env.DB.prepare("SELECT result FROM idempotency WHERE operation_id=? LIMIT 1").bind(envelope.operation_id).first<{result:string}>();if(prior)return json(safeJson(prior.result));
  const targetId=String(target.id),targetType=String(target.type),payload=envelope.payload??{};const existing=await env.DB.prepare("SELECT version,lifecycle_epoch,authority_epoch FROM aggregates WHERE id=? AND organization_id=? LIMIT 1").bind(targetId,session.organization_id).first<{version:number;lifecycle_epoch:number;authority_epoch:number}>();
  const expected=Number(envelope.expected_version??0);if(existing&&expected!==existing.version)return json({success:false,operation_id:envelope.operation_id,events:[],error:{code:"VERSION_CONFLICT",category:"CONCURRENCY",retryability:"RETRYABLE",safe_details:{expected,actual:existing.version},correlation_id:String(envelope.correlation_id??envelope.operation_id)}},409);
  const destructive=/delete|remove|reject|decline|end|cancel/i.test(String(envelope.command_type));const nextVersion=(existing?.version??0)+1;
  if(destructive&&existing)await env.DB.prepare("DELETE FROM aggregates WHERE id=? AND organization_id=?").bind(targetId,session.organization_id).run();
  else {const previous=existing?await env.DB.prepare("SELECT state FROM aggregates WHERE id=?").bind(targetId).first<{state:string}>():null;const merged={...((previous?safeJson(previous.state):{}) as Json),...((payload as Json)),_command_type:envelope.command_type,_updated_by:session.sub};await env.DB.prepare("INSERT INTO aggregates(id,aggregate_type,version,lifecycle_epoch,authority_epoch,state,updated_at,organization_id) VALUES(?,?,?,?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET version=excluded.version,lifecycle_epoch=excluded.lifecycle_epoch,authority_epoch=excluded.authority_epoch,state=excluded.state,updated_at=excluded.updated_at").bind(targetId,targetType,nextVersion,Number(envelope.expected_lifecycle_epoch??0),Number(envelope.expected_authority_epoch??0),JSON.stringify(merged),Date.now(),session.organization_id).run();}
  const event={event_id:uuid(),aggregate_id:targetId,aggregate_type:targetType,event_type:envelope.command_type,aggregate_version:nextVersion,organization_id:session.organization_id,payload,occurred_at:new Date().toISOString(),actor:{user_id:session.sub,organization_id:session.organization_id}};
  await env.DB.prepare("INSERT INTO domain_events(event_id,aggregate_id,aggregate_version,event_type,payload,occurred_at,vector_clock,operation_id,correlation_id,actor,organization_id) VALUES(?,?,?,?,?,?,?,?,?,?,?)").bind(event.event_id,targetId,nextVersion,envelope.command_type,JSON.stringify(payload),Date.now(),"{}",String(envelope.operation_id),String(envelope.correlation_id??envelope.operation_id),JSON.stringify(event.actor),session.organization_id).run();
  const result={success:true,operation_id:String(envelope.operation_id),new_version:nextVersion,new_lifecycle_epoch:Number(envelope.expected_lifecycle_epoch??0),new_authority_epoch:Number(envelope.expected_authority_epoch??0),events:[event],result:{id:targetId,type:targetType,state:payload}};await env.DB.prepare("INSERT INTO idempotency(operation_id,result,created_at) VALUES(?,?,?)").bind(envelope.operation_id,JSON.stringify(result),Date.now()).run();await audit(env,session,envelope.command_type,targetId,{correlation_id:String(envelope.correlation_id??envelope.operation_id)});return json(result);
}
async function hierarchy(env:Env,session:Session){const rows=await env.DB.prepare("SELECT id,username,email,organization_id,is_admin,is_active,class,role,parent_user_id FROM users WHERE organization_id=? ORDER BY username").bind(session.organization_id).all();return json({users:rows.results??[]});}
async function profiles(env:Env,session:Session,ownerId?:string){const rows=ownerId?await env.DB.prepare("SELECT id,state FROM aggregates WHERE organization_id=? AND aggregate_type=? AND id=? LIMIT 1").bind(session.organization_id,"staff_profile",ownerId).all<Record<string,unknown>>():await env.DB.prepare("SELECT id,state FROM aggregates WHERE organization_id=? AND aggregate_type=? ORDER BY updated_at DESC LIMIT 100").bind(session.organization_id,"staff_profile").all<Record<string,unknown>>();return json((rows.results??[]).map(r=>({id:r.id,...(safeJson(String(r.state)) as Json)})));}
async function pushSubscribe(request:Request,env:Env,session:Session){const body=await request.json().catch(()=>null) as Json|null;if(!body||typeof body.endpoint!=="string"||typeof body.p256dh!=="string"||typeof body.auth!=="string")return json({error:"INVALID_PUSH_SUBSCRIPTION"},400);const id=typeof body.id==="string"?body.id:uuid();await env.DB.prepare("INSERT INTO push_subscriptions(id,user_id,organization_id,endpoint,p256dh,auth,platform,created_at) VALUES(?,?,?,?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET endpoint=excluded.endpoint,p256dh=excluded.p256dh,auth=excluded.auth,platform=excluded.platform").bind(id,session.sub,session.organization_id,body.endpoint,body.p256dh,body.auth,String(body.platform??"web"),Date.now()).run();return json({id,success:true});}
async function pushDelete(env:Env,session:Session,id:string){await env.DB.prepare("DELETE FROM push_subscriptions WHERE id=? AND user_id=? AND organization_id=?").bind(id,session.sub,session.organization_id).run();return json({success:true});}
async function fileDownload(env:Env,session:Session,hash:string){const asset=await env.DB.prepare("SELECT blob_key,content_type FROM file_assets WHERE content_hash=? AND organization_id=? LIMIT 1").bind(hash,session.organization_id).first<{blob_key:string;content_type:string|null}>();if(!asset)return json({error:"FILE_NOT_FOUND"},404);const endpoint=env.HF_S3_ENDPOINT,bucket=env.HF_S3_BUCKET,access=env.HF_S3_ACCESS_KEY_ID,secret=env.HF_S3_SECRET_ACCESS_KEY;if(!endpoint||!bucket||!access||!secret)return json({error:"BLOB_STORE_NOT_CONFIGURED"},503);const response=await signedS3Get(endpoint,bucket,asset.blob_key,access,secret);if(!response.ok)return json({error:"BLOB_STORE_UNAVAILABLE"},502);const headers=new Headers();headers.set("cache-control","private, max-age=60");if(asset.content_type)headers.set("content-type",asset.content_type);return new Response(response.body,{status:200,headers});}
async function hmacBytes(keyBytes:Uint8Array,value:string):Promise<Uint8Array>{const key=await crypto.subtle.importKey("raw",keyBytes,{name:"HMAC",hash:"SHA-256"},false,["sign"]);return new Uint8Array(await crypto.subtle.sign("HMAC",key,new TextEncoder().encode(value)));}
async function signedS3Get(endpoint:string,bucket:string,key:string,access:string,secret:string){const base=endpoint.replace(/\/$/,"");const path=`/${bucket}/${key.split("/").map(encodeURIComponent).join("/")}`;const url=`${base}${path}`;const host=new URL(base).host;const bodyHash=await sha256("");const date=new Date();const amzDate=date.toISOString().replace(/[-:]/g,"").replace(/\.\d{3}Z$/,"");const short=amzDate.slice(0,8);const canonicalHeaders=`host:${host}\nx-amz-content-sha256:${bodyHash}\nx-amz-date:${amzDate}\n`;const signedHeaders="host;x-amz-content-sha256;x-amz-date";const canonicalRequest=`GET\n${path}\n\n${canonicalHeaders}\n${signedHeaders}\n${bodyHash}`;const scope=`${short}/us-east-1/s3/aws4_request`;const stringToSign=`AWS4-HMAC-SHA256\n${amzDate}\n${scope}\n${await sha256(canonicalRequest)}`;const kDate=await hmacBytes(new TextEncoder().encode(`AWS4${secret}`),short);const kRegion=await hmacBytes(kDate,"us-east-1");const kService=await hmacBytes(kRegion,"s3");const signingKey=await hmacBytes(kService,"aws4_request");const signature=Array.from(await hmacBytes(signingKey,stringToSign),(b)=>b.toString(16).padStart(2,"0")).join("");return fetch(url,{headers:{Host:host,"x-amz-content-sha256":bodyHash,"x-amz-date":amzDate,Authorization:`AWS4-HMAC-SHA256 Credential=${access}/${scope}, SignedHeaders=${signedHeaders}, Signature=${signature}`}});}

async function dispatch(request:Request,env:Env):Promise<Response>{
  const url=new URL(request.url);const origin=corsOrigin(request,env);
  if(request.method==="OPTIONS")return new Response(null,{status:204,headers:{"access-control-allow-origin":origin??"*","access-control-allow-methods":"GET,POST,PUT,DELETE,OPTIONS","access-control-allow-headers":"Authorization,Content-Type","access-control-allow-credentials":"true","access-control-max-age":"86400"}});
  if(url.pathname==="/health")return json({status:"ok",service:"onyx-cloudflare-worker",runtime:"cloudflare-workers-free"},200,origin);
  if(url.pathname==="/ready"){try{await env.DB.prepare("SELECT 1").first();return json({status:"ok",service:"onyx-cloudflare-worker",database:"d1"},200,origin);}catch{return json({status:"degraded",database:"unavailable"},503,origin);}}
  try{
    if(url.pathname==="/api/auth/login"&&request.method==="POST")return authLogin(request,env);
    if((url.pathname==="/api/auth/supabase"||url.pathname==="/api/auth/clerk")&&request.method==="POST")return authSupabaseAllFather(request,env);
    if(url.pathname==="/api/auth/refresh"&&request.method==="POST")return authRefresh(request,env);
    if(url.pathname==="/api/auth/logout"&&request.method==="POST")return authLogout(request,env);
    const session=await requireSession(request,env);
    if(url.pathname==="/api/query"&&request.method==="GET")return queryRoute(request,env,session);
    if(url.pathname==="/api/command"&&request.method==="POST")return commandRoute(request,env,session);
    if(url.pathname==="/api/users/hierarchy"&&request.method==="GET")return hierarchy(env,session);
    if(url.pathname==="/api/profiles"&&request.method==="GET")return profiles(env,session);
    if(url.pathname.startsWith("/api/profiles/")&&request.method==="GET")return profiles(env,session,url.pathname.split("/").pop());
    if(url.pathname==="/api/push/subscriptions"&&request.method==="POST")return pushSubscribe(request,env,session);
    if(url.pathname.startsWith("/api/push/subscriptions/")&&request.method==="DELETE")return pushDelete(env,session,url.pathname.split("/").pop()!);
    if(url.pathname.startsWith("/api/files/")&&request.method==="GET")return fileDownload(env,session,url.pathname.split("/").pop()!);
    if(url.pathname==="/api/events"&&request.method==="GET"){const rows=await env.DB.prepare("SELECT event_id,aggregate_id,event_type,payload,occurred_at,organization_id FROM domain_events WHERE organization_id=? ORDER BY occurred_at DESC LIMIT 100").bind(session.organization_id).all();return json(rows.results??[],200,origin);}
    if(url.pathname==="/api/allfather/organizations"&&request.method==="POST")return createOrganization(request,env,session);
    if(url.pathname==="/api/allfather/admins"&&request.method==="POST")return createUser(request,env,session,"ORGANIZATION_ADMIN");
    if(url.pathname==="/api/admin/staff"&&request.method==="POST")return createUser(request,env,session,"STAFF");
    if(url.pathname.startsWith("/api/admin/users/")&&request.method==="DELETE")return deactivateUser(request,env,session,url.pathname.split("/").pop()!);
    return json({error:"ROUTE_NOT_MIGRATED",route:url.pathname,message:"The route exists in the Axum reference service but has not yet been ported with equivalent Cloudflare semantics."},501,origin);
  }catch(error){const message=error instanceof Error?error.message:"internal error";if(message==="AUTH_REQUIRED"||message.includes("token")||message.includes("expired")||message.includes("revoked"))return json({error:"UNAUTHORIZED",category:"AUTHORITY",retryability:"NON_RETRYABLE"},401,origin);if(message==="FORBIDDEN_ROLE")return json({error:"FORBIDDEN",category:"AUTHORITY",retryability:"NON_RETRYABLE"},403,origin);if(message.startsWith("INVALID_")||message==="PASSWORD_TOO_SHORT"||message==="ORGANIZATION_NOT_FOUND")return json({error:message,category:"DOMAIN",retryability:"NON_RETRYABLE"},400,origin);console.error(error);return json({error:"INTERNAL_SERVER_ERROR",category:"INFRASTRUCTURE",retryability:"TRANSIENT"},500,origin);}
}
export default {fetch:dispatch};
