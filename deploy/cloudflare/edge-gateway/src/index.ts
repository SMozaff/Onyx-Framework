export { EventHub, RelayHub } from "./event-hub";
import { argon2id, argon2Verify } from "hash-wasm";
import { getSupabaseUser, verifySupabaseJwt } from "./supabase";
import { missingAuthSchema } from "./schema-contract";
import { authorizeCommand, authorizeQuery, ONYX_ROLES, payloadOwnerIds, resourceOwnedBy } from "./authorization";
import { validateCommandSemantics } from "./command-semantics";
import { staffOwnedPredicateSql, staffOwnedBindings, resourceIdFromReference } from "./resource-scope";

type Json = Record<string, unknown>;

interface Env {
  DB: D1Database;
  EVENTS_HUB: DurableObjectNamespace;
  RELAY_HUB: DurableObjectNamespace;
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
  iat_ms?: number;
  jti: string;
  sid: string;
  typ: "access" | "refresh";
  /** Verified Supabase subject; present only on All-Father sessions. */
  identity_sub?: string;
  iss?: string;
  aud?: string | string[];
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
const ONYX_JWT_ISSUER = "onyx-cloudflare-worker";
const ONYX_JWT_AUDIENCE = "onyx-api";

async function jwtSign(env:Env,session:Omit<Session,"jti">):Promise<string>{
  if(!env.ONYX_JWT_SECRET) throw new Error("ONYX_JWT_SECRET is not configured");
  const header=b64u(JSON.stringify({alg:"HS256",typ:"JWT"}));
  const payload=b64u(JSON.stringify({...session,iat_ms:Date.now(),iss:ONYX_JWT_ISSUER,aud:ONYX_JWT_AUDIENCE,jti:uuid()}));
  const input=`${header}.${payload}`;
  return `${input}.${b64u(await hmac(env.ONYX_JWT_SECRET,input))}`;
}

async function jwtVerify(
  env:Env,
  token:string,
  options:{allowExpired?:boolean;allowRevoked?:boolean}={}
):Promise<Session>{
  if(!env.ONYX_JWT_SECRET) throw new Error("ONYX_JWT_SECRET is not configured");
  if(typeof token!=="string"||token.length>16384||!token)throw new Error("invalid token");
  const parts=token.split(".");
  if(parts.length!==3||parts.some(part=>!part||!/^[A-Za-z0-9_-]+$/.test(part)))throw new Error("invalid token");
  let header:Json,payload:Json,signature:Uint8Array;
  try{
    header=decodeJsonB64u(parts[0]);
    payload=decodeJsonB64u(parts[1]);
    signature=fromB64u(parts[2]);
  }catch{
    throw new Error("invalid token");
  }
  if(!header||typeof header!=="object"||Array.isArray(header)||
     !payload||typeof payload!=="object"||Array.isArray(payload))
    throw new Error("invalid token");
  if(header.alg!=="HS256"||header.typ!=="JWT")throw new Error("invalid token");
  let validSignature=false;
  try{
    const key=await crypto.subtle.importKey("raw",new TextEncoder().encode(env.ONYX_JWT_SECRET),{name:"HMAC",hash:"SHA-256"},false,["verify"]);
    validSignature=await crypto.subtle.verify("HMAC",key,signature,new TextEncoder().encode(`${parts[0]}.${parts[1]}`));
  }catch{
    throw new Error("invalid token");
  }
  if(!validSignature)throw new Error("invalid token");
  const session=payload as unknown as Session;
  const validAudience=typeof session.aud==="string"
    ?session.aud===ONYX_JWT_AUDIENCE
    :Array.isArray(session.aud)&&session.aud.includes(ONYX_JWT_AUDIENCE);
  if(session.iss!==ONYX_JWT_ISSUER||!validAudience)throw new Error("invalid token");
  if(typeof session.sub!=="string"||!session.sub||typeof session.organization_id!=="string"||!session.organization_id||
     typeof session.jti!=="string"||!session.jti||typeof session.sid!=="string"||!session.sid||session.sid.length>128||!Number.isSafeInteger(session.exp)||session.exp<=0||
     (session.typ!=="access"&&session.typ!=="refresh")||
     typeof session.role!=="string"||typeof session.client_type!=="string"||
     (typeof session.is_admin!=="boolean"&&session.is_admin!==0&&session.is_admin!==1))
    throw new Error("invalid token");
  if(!options.allowExpired&&session.exp<=now())throw new Error("expired token");
  if(!options.allowRevoked){
    const revoked=await env.DB.prepare("SELECT 1 FROM token_revocations WHERE token_hash=? LIMIT 1").bind(await sha256(token)).first();
    const revokedSession=await env.DB.prepare("SELECT 1 FROM token_revocations WHERE token_hash=? LIMIT 1").bind(await sha256("session:"+session.sid)).first();
    if(revoked||revokedSession)throw new Error("revoked token");
  }
  return session;
}
function authorization(request:Request){const value=request.headers.get("authorization");return value?value.replace(/^Bearer\s+/i,""):null;}
async function requireSession(request:Request,env:Env){
  const token=authorization(request);
  if(!token)throw new Error("AUTH_REQUIRED");
  const session=await jwtVerify(env,token);
  // Refresh tokens are only valid at /api/auth/refresh, never as bearer credentials.
  if(session.typ!=="access") throw new Error("AUTH_REQUIRED");
  // Account and organization status, role and tenant are re-read on EVERY
  // protected request. Long-lived JWT claims are not authority.
  const current=await env.DB.prepare(
    "SELECT u.id,u.username,u.organization_id,u.is_active,u.role,u.is_admin,u.class,u.supabase_user_id FROM users u JOIN organizations o ON o.id=u.organization_id AND o.is_active=1 WHERE u.id=? AND u.is_active=1 LIMIT 1"
  ).bind(session.sub).first<Record<string,unknown>>();
  const currentRole=current?roleForUser(current):"";
  if(!current||!ONYX_ROLES.includes(currentRole as typeof ONYX_ROLES[number])||(String(current.id)==="__allfather__"&&currentRole!=="ALL_FATHER"))throw new Error("AUTH_REQUIRED");
  const revocation=await env.DB.prepare("SELECT revoked_before_ms FROM user_session_revocations WHERE user_id=? LIMIT 1").bind(String(current.id)).first<{revoked_before_ms:number}>();
  if(revocation && (!Number.isSafeInteger(session.iat_ms) || Number(session.iat_ms)<=Number(revocation.revoked_before_ms)))throw new Error("revoked session");
  if(String(current.organization_id)!==session.organization_id||currentRole!==session.role||
     Boolean(current.is_admin)!==Boolean(session.is_admin))throw new Error("AUTH_REQUIRED");
  // All-Father sessions are bound to the canonical ONYX principal and exact
  // provisioned Supabase subject. Removed/rebound mappings invalidate sessions.
  if(currentRole==="ALL_FATHER"){
    if(String(current.id)!=="__allfather__"||!current.is_admin||!current.supabase_user_id||
       session.identity_sub!==String(current.supabase_user_id))throw new Error("AUTH_REQUIRED");
  }else if(session.identity_sub){
    throw new Error("AUTH_REQUIRED");
  }
  return session;
}
function corsOrigin(request:Request,env:Env){const origin=request.headers.get("origin");const configured=env.ONYX_CORS_ORIGINS?.split(",").map(x=>x.trim()).filter(Boolean)??[];if(!origin)return null;return configured.includes(origin)&&!configured.includes("*")?origin:null;}

async function verifyPassword(password:string,encoded:string):Promise<boolean>{
  if(!encoded.startsWith("$argon2id$")) return false;
  try { return await argon2Verify({password,hash:encoded}); } catch { return false; }
}
async function hashPassword(password:string):Promise<string>{
  const salt=crypto.getRandomValues(new Uint8Array(16));
  return String(await argon2id({password,salt,parallelism:1,iterations:3,memorySize:65536,hashLength:32,outputType:"encoded"}));
}
function clientType(payload:Json|null){return validClientType(payload);}
function roleForUser(user:Record<string,unknown>):string{
  // Never infer a role from mutable profile values or the legacy is_admin bit.
  // Invalid/missing D1 roles fail closed and cannot receive a usable session.
  const explicit=typeof user.role==="string"?user.role:"";
  return ONYX_ROLES.includes(explicit as typeof ONYX_ROLES[number])?explicit:"";
}
async function issuePair(env:Env,user:Record<string,unknown>,type:string,identitySub?:string,sessionId?:string){
  const role=roleForUser(user);
  const base={sub:String(user.id),username:String(user.username),organization_id:String(user.organization_id),is_admin:Boolean(user.is_admin),class:(user.class as string|null)??null,role,client_type:type,sid:sessionId??uuid(),...(role==="ALL_FATHER"&&identitySub?{identity_sub:identitySub}:{})};
  const access=await jwtSign(env,{...base,exp:now()+3600,typ:"access"}); const refresh=await jwtSign(env,{...base,exp:now()+7*86400,typ:"refresh"});
  return {access_token:access,refresh_token:refresh,expires_in:3600,user:{id:user.id,username:user.username,email:user.email??null,organization_id:user.organization_id,is_admin:Boolean(user.is_admin),class:user.class??null,role}};
}
async function authLogin(request:Request,env:Env){
  const body=await request.json().catch(()=>null) as Json|null;
  const identifier=typeof body?.username==="string"?body.username.trim():typeof body?.email==="string"?body.email.trim():"";
  const password=typeof body?.password==="string"?body.password:"";
  if(!identifier||!password)return json({error:"INVALID_CREDENTIALS",category:"AUTHORITY",retryability:"NON_RETRYABLE"},401);
  const user=await env.DB.prepare("SELECT u.id,u.username,u.email,u.organization_id,u.password_hash,u.is_admin,u.is_active,u.class,u.role,u.supabase_user_id FROM users u JOIN organizations o ON o.id=u.organization_id AND o.is_active=1 WHERE (LOWER(u.username)=LOWER(?) OR LOWER(u.email)=LOWER(?)) AND u.is_active=1 LIMIT 1").bind(identifier,identifier).first<Record<string,unknown>>();
  const role=user?roleForUser(user):"";
  if(!user||!user.is_active||!ONYX_ROLES.includes(role as typeof ONYX_ROLES[number])||role==="ALL_FATHER"||String(user.id)==="__allfather__"||typeof user.password_hash!=="string"||!(await verifyPassword(password,String(user.password_hash))))return json({error:"INVALID_CREDENTIALS",category:"AUTHORITY",retryability:"NON_RETRYABLE"},401);
  const requestedClientType = clientType(body);
  if (!requestedClientType) return json({error:"INVALID_CLIENT_TYPE",category:"DOMAIN",retryability:"NON_RETRYABLE"},400);
  // client_type is caller-controlled metadata, never a trustworthy basis for an
  // authorization decision. The persisted mobile class policy is exposed to
  // administrators but is not enforced until the request carries a
  // server-verifiable client identity/attestation.
  return json(await issuePair(env,user,requestedClientType));
}
async function authRefresh(request:Request,env:Env){
  const body=await request.json().catch(()=>null) as Json|null;
  const token=typeof body?.refresh_token==="string"?body.refresh_token:"";
  try{
    const claims=await jwtVerify(env,token);
    if(claims.typ!=="refresh")throw new Error("wrong token type");
    const user=await env.DB.prepare(
      "SELECT u.id,u.username,u.email,u.organization_id,u.is_admin,u.is_active,u.class,u.role,u.supabase_user_id FROM users u JOIN organizations o ON o.id=u.organization_id AND o.is_active=1 WHERE u.id=? AND u.is_active=1 LIMIT 1"
    ).bind(claims.sub).first<Record<string,unknown>>();
    if(!user)throw new Error("inactive user or organization");
    const role=roleForUser(user);
    if(!ONYX_ROLES.includes(role as typeof ONYX_ROLES[number])||role!==claims.role||
       String(user.organization_id)!==claims.organization_id||
       Boolean(user.is_admin)!==Boolean(claims.is_admin))throw new Error("session authority changed");
    if(role==="ALL_FATHER"){
      if(String(user.id)!=="__allfather__"||!user.is_admin||!user.supabase_user_id||
         claims.identity_sub!==String(user.supabase_user_id))throw new Error("All-Father mapping changed");
    }else if(claims.identity_sub)throw new Error("unexpected external identity");
    // Consume the refresh token exactly once. Fail closed if D1 cannot prove
    // this invocation inserted the revocation record (including replay races).
    const consumed=await env.DB.prepare(
      "INSERT OR IGNORE INTO token_revocations(token_hash,revoked_at) VALUES(?,?)"
    ).bind(await sha256(token),now()).run();
    if(Number(consumed.meta?.changes??0)!==1)throw new Error("refresh token replayed");
    return json(await issuePair(env,user,typeof claims.client_type==="string"?claims.client_type:"web",
      role==="ALL_FATHER"?String(user.supabase_user_id):undefined,claims.sid));
  }catch{
    return json({error:"INVALID_REFRESH_TOKEN",category:"AUTHORITY",retryability:"NON_RETRYABLE"},401);
  }
}
async function authLogout(request:Request,env:Env){
  const bearer=authorization(request);
  const rawBody=await request.json().catch(()=>null);
  const body=rawBody&&typeof rawBody==="object"&&!Array.isArray(rawBody)?rawBody as Json:{};
  let bearerSession:Session|null=null;
  let refreshSession:Session|null=null;
  const refreshToken=typeof body.refresh_token==="string"?body.refresh_token:"";
  try{if(bearer)bearerSession=await jwtVerify(env,bearer,{allowExpired:true,allowRevoked:true});}catch{}
  try{if(refreshToken)refreshSession=await jwtVerify(env,refreshToken,{allowExpired:true,allowRevoked:true});}catch{}
  const tokensToRevoke=new Set<string>();
  const sessionsToRevoke=new Set<string>();
  if(bearer&&bearerSession){tokensToRevoke.add(bearer);sessionsToRevoke.add(bearerSession.sid);}
  // Refresh-only logout is supported. When both tokens are supplied they
  // must belong to the same session family.
  if(refreshToken&&refreshSession&&(!bearerSession||refreshSession.sid===bearerSession.sid)){
    tokensToRevoke.add(refreshToken);
    sessionsToRevoke.add(refreshSession.sid);
  }
  for(const token of tokensToRevoke){
    await env.DB.prepare("INSERT OR IGNORE INTO token_revocations(token_hash,revoked_at) VALUES(?,?)")
      .bind(await sha256(token),now()).run();
  }
  for(const sid of sessionsToRevoke){
    await env.DB.prepare("INSERT OR IGNORE INTO token_revocations(token_hash,revoked_at) VALUES(?,?)")
      .bind(await sha256("session:"+sid),now()).run();
  }
  return json({success:true});
}

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
    // Use the immutable canonical principal ID. Username/email are mutable
    // profile attributes and must never select the privileged identity.
    const user=await env.DB.prepare("SELECT u.id,u.username,u.email,u.supabase_user_id,u.organization_id,u.is_admin,u.is_active,u.class,u.role FROM users u JOIN organizations o ON o.id=u.organization_id AND o.is_active=1 WHERE u.id=? AND u.is_active=1 LIMIT 1").bind("__allfather__").first<Record<string,unknown>>();
    if(!user?.is_active||roleForUser(user)!=="ALL_FATHER"||!user.is_admin)return json({error:"ALLFATHER_NOT_PROVISIONED",category:"AUTHORITY",retryability:"NON_RETRYABLE"},403);
    // Contract (docs/SECURITY_AUTHENTICATION_MODEL.md): the Supabase subject
    // mapping is written only by an owner-controlled provisioning process.
    // Login must never create, update, or replace users.supabase_user_id; an
    // unmapped or mismatched subject is denied without a database write.
    if(!user.supabase_user_id||String(user.supabase_user_id)!==String(claims.sub)){
      console.warn(JSON.stringify({event:"allfather_subject_rejected",reason:user.supabase_user_id?"SUBJECT_MISMATCH":"SUBJECT_UNMAPPED",principal_id:String(user.id)}));
      return json({error:"ALLFATHER_NOT_PROVISIONED",category:"AUTHORITY",retryability:"NON_RETRYABLE"},403);
    }
    return json(await issuePair(env,user,"allfather",String(claims.sub)));
  }catch(error){
    // A bad/expired token is an authentication failure. Provider outages and
    // missing runtime configuration are infrastructure failures and must not
    // mislead clients into treating them as invalid credentials.
    const rawReason=error instanceof Error?error.message:"";
    const infrastructureReasons=[
      "SUPABASE_AUTH_UNAVAILABLE",
      "SUPABASE_URL is not configured",
      "SUPABASE_PUBLISHABLE_KEY is not configured",
      "ONYX_JWT_SECRET is not configured",
      "SUPABASE_JWKS_URL is not configured",
    ];
    const infrastructureFailure=infrastructureReasons.includes(rawReason)||error instanceof TypeError;
    // Log only an allowlisted classification, never raw provider/error text
    // that might accidentally contain request credentials or token material.
    const reasonCode=infrastructureReasons.includes(rawReason)?rawReason:
      error instanceof TypeError?"PROVIDER_OR_TOKEN_PARSING_ERROR":"AUTHENTICATION_FAILURE";
    console.error(JSON.stringify({event:"allfather_exchange_failed",reason:reasonCode}));
    if(infrastructureFailure){
      return json({error:"AUTHENTICATION_SERVICE_UNAVAILABLE",category:"INFRASTRUCTURE",retryability:"TRANSIENT"},502);
    }
    return json({error:"INVALID_SUPABASE_TOKEN",category:"AUTHORITY",retryability:"NON_RETRYABLE"},401);
  }
}

const RELAY_TICKET_TTL_SECONDS=30;
type RelayTicketClaims={sub:string;organization_id:string;typ:"relay_ticket";target_id:string;self_replica:string;iat:number;exp:number;jti:string};
async function signRelayTicket(env:Env,claims:RelayTicketClaims):Promise<string>{
  if(!env.ONYX_JWT_SECRET)throw new Error("ONYX_JWT_SECRET is not configured");
  const header=b64u(JSON.stringify({alg:"HS256",typ:"JWT"})),payload=b64u(JSON.stringify(claims)),input=header+"."+payload;
  return input+"."+b64u(await hmac(env.ONYX_JWT_SECRET,input));
}
async function verifyRelayTicket(env:Env,token:string):Promise<RelayTicketClaims>{
  if(!env.ONYX_JWT_SECRET)throw new Error("ONYX_JWT_SECRET is not configured");
  if(token.length>8192)throw new Error("invalid relay ticket");
  const parts=token.split(".");
  if(parts.length!==3||parts.some(part=>!/^[A-Za-z0-9_-]+$/.test(part)))throw new Error("invalid relay ticket");
  const header=decodeJsonB64u(parts[0]),claims=decodeJsonB64u(parts[1]);
  if(header.alg!=="HS256"||header.typ!=="JWT")throw new Error("invalid relay ticket");
  const key=await crypto.subtle.importKey("raw",new TextEncoder().encode(env.ONYX_JWT_SECRET),{name:"HMAC",hash:"SHA-256"},false,["verify"]);
  if(!await crypto.subtle.verify("HMAC",key,fromB64u(parts[2]),new TextEncoder().encode(parts[0]+"."+parts[1])))throw new Error("invalid relay ticket");
  const c=claims as unknown as RelayTicketClaims;
  if(c.typ!=="relay_ticket"||!looksLikeUuid(c.sub)||!looksLikeUuid(c.organization_id)||!looksLikeUuid(c.target_id)||!looksLikeUuid(c.self_replica)||typeof c.jti!=="string"||!c.jti||!Number.isSafeInteger(c.exp)||c.exp<=now()||!Number.isSafeInteger(c.iat)||c.iat>now()+5||c.exp-c.iat>RELAY_TICKET_TTL_SECONDS)throw new Error("invalid relay ticket");
  return c;
}
async function issueRelayTicket(request:Request,env:Env,session:Session){
  if(session.client_type==="mobile_observer")return json({error:"CLIENT_CAPABILITY_REQUIRED",category:"AUTHORITY",retryability:"NON_RETRYABLE"},403);
  const body=await request.json().catch(()=>null) as Json|null;
  if(!body||Array.isArray(body)||Object.keys(body).some(key=>key!=="target_id"&&key!=="self_replica")||
     typeof body.target_id!=="string"||!looksLikeUuid(body.target_id)||typeof body.self_replica!=="string"||!looksLikeUuid(body.self_replica))
    return json({error:"INVALID_REPLICA_ID",category:"DOMAIN",retryability:"NON_RETRYABLE"},400);
  if(!looksLikeUuid(session.sub)||!looksLikeUuid(session.organization_id))return json({error:"UNAUTHORIZED",category:"AUTHORITY",retryability:"NON_RETRYABLE"},401);
  const self=body.self_replica.toLowerCase(),target=body.target_id.toLowerCase(),timestamp=Date.now();
  try{
    await env.DB.prepare("INSERT INTO replica_ownership(replica_id,user_id,organization_id,claimed_at) VALUES(?,?,?,?) ON CONFLICT(replica_id) DO NOTHING")
      .bind(self,session.sub,session.organization_id,now()).run();
    const owner=await env.DB.prepare("SELECT user_id,organization_id FROM replica_ownership WHERE replica_id=? LIMIT 1").bind(self).first<{user_id:string;organization_id:string}>();
    if(!owner||owner.user_id!==session.sub||owner.organization_id!==session.organization_id)return json({error:"REPLICA_OWNERSHIP_REJECTED",category:"AUTHORITY",retryability:"NON_RETRYABLE"},401);
    const claims:RelayTicketClaims={sub:session.sub,organization_id:session.organization_id,typ:"relay_ticket",target_id:target,self_replica:self,iat:now(),exp:now()+RELAY_TICKET_TTL_SECONDS,jti:uuid()};
    return json({ticket:await signRelayTicket(env,claims),expires_in:RELAY_TICKET_TTL_SECONDS});
  }catch{
    return json({error:"RELAY_TICKET_UNAVAILABLE",category:"INFRASTRUCTURE",retryability:"TRANSIENT"},503);
  }
}
async function relaySocket(request:Request,env:Env):Promise<Response>{
  if(request.headers.get("upgrade")?.toLowerCase()!=="websocket")return json({error:"WEBSOCKET_UPGRADE_REQUIRED",category:"DOMAIN",retryability:"NON_RETRYABLE"},426);
  const url=new URL(request.url),target=url.pathname.split("/").pop()||"",token=url.searchParams.get("ticket")||"";
  if(!looksLikeUuid(target)||!token)return json({error:"UNAUTHORIZED",category:"AUTHORITY",retryability:"NON_RETRYABLE"},401);
  let claims:RelayTicketClaims;
  try{claims=await verifyRelayTicket(env,token);}catch{return json({error:"UNAUTHORIZED",category:"AUTHORITY",retryability:"NON_RETRYABLE"},401);}
  if(claims.target_id!==target.toLowerCase())return json({error:"RELAY_TARGET_MISMATCH",category:"AUTHORITY",retryability:"NON_RETRYABLE"},403);
  try{
    const redeemed=await env.DB.prepare("INSERT OR IGNORE INTO relay_ticket_redemptions(jti,expires_at,redeemed_at) VALUES(?,?,?)")
      .bind(claims.jti,claims.exp,now()).run();
    if(Number(redeemed.meta?.changes??0)!==1)return json({error:"RELAY_TICKET_REPLAYED",category:"AUTHORITY",retryability:"NON_RETRYABLE"},401);
    await env.DB.prepare("DELETE FROM relay_ticket_redemptions WHERE expires_at<?").bind(now()-3600).run();
    const headers=new Headers(request.headers);
    headers.set("x-onyx-relay-org",claims.organization_id);
    headers.set("x-onyx-relay-user",claims.sub);
    headers.set("x-onyx-relay-self",claims.self_replica);
    headers.set("x-onyx-relay-target",claims.target_id);
    const hub=env.RELAY_HUB.idFromName(claims.organization_id);
    return await env.RELAY_HUB.get(hub).fetch(new Request(request,{headers}));
  }catch{
    return json({error:"RELAY_UNAVAILABLE",category:"INFRASTRUCTURE",retryability:"TRANSIENT"},503);
  }
}

function requireRole(session:Session,...roles:string[]){if(!roles.includes(session.role))throw new Error("FORBIDDEN_ROLE");}
function requiredString(body:Json,key:string):string{const value=body[key];if(typeof value!=="string"||!value.trim())throw new Error(`INVALID_${key.toUpperCase()}`);return value.trim();}
function optionalEmail(body:Json):string|null{const value=typeof body.email==="string"?body.email.trim().toLowerCase():null;return value||null;}
async function createOrganization(request:Request,env:Env,session:Session){
  requireRole(session,"ALL_FATHER");const body=await request.json().catch(()=>null) as Json|null;if(!body)throw new Error("INVALID_BODY");
  const name=requiredString(body,"name");const id=typeof body.id==="string"&&body.id.trim()?body.id.trim():uuid();const timestamp=Date.now();
  const create=env.DB.prepare("INSERT INTO organizations(id,name,is_active,created_at,updated_at) VALUES(?,?,1,?,?)")
    .bind(id,name,timestamp,timestamp);
  const auditStatement=env.DB.prepare("INSERT INTO audit_log(organization_id,user_id,action,correlation_id,details,created_at) SELECT ?,?,?,?,?,? WHERE changes()=1")
    .bind(session.organization_id,session.sub,"organization.create",uuid(),JSON.stringify({target:id,name}),timestamp);
  try{
    const result=await env.DB.batch([create,auditStatement]);
    if(Number(result[0]?.meta?.changes??0)!==1)return json({error:"ORGANIZATION_EXISTS",category:"DOMAIN",retryability:"NON_RETRYABLE"},409);
    return json({id,name,is_active:true});
  }catch(error){
    const message=error instanceof Error?error.message:"";
    if(/unique constraint|already exists/i.test(message))return json({error:"ORGANIZATION_EXISTS",category:"DOMAIN",retryability:"NON_RETRYABLE"},409);
    throw error;
  }
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
  const id=uuid(),timestamp=Date.now(),isAdmin=targetRole==="ORGANIZATION_ADMIN"?1:0,parent=targetRole==="STAFF"?session.sub:"__allfather__";
  const passwordHash=await hashPassword(password);
  const action=targetRole==="STAFF"?"staff.create":"admin.create";
  const create=env.DB.prepare("INSERT INTO users(id,username,email,organization_id,password_hash,is_admin,is_active,class,role,parent_user_id,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?,?,?,?,?)")
    .bind(id,username,email,organizationId,passwordHash,isAdmin,1,null,targetRole,parent,timestamp,timestamp);
  const auditStatement=env.DB.prepare("INSERT INTO audit_log(organization_id,user_id,action,correlation_id,details,created_at) SELECT ?,?,?,?,?,? WHERE changes()=1")
    .bind(session.organization_id,session.sub,action,uuid(),JSON.stringify({target:id,username,email,organization_id:organizationId}),timestamp);
  try{
    const result=await env.DB.batch([create,auditStatement]);
    if(Number(result[0]?.meta?.changes??0)!==1)return json({error:"USER_EXISTS",category:"DOMAIN",retryability:"NON_RETRYABLE"},409);
  }catch(error){
    const message=error instanceof Error?error.message:"";
    if(/unique constraint|already exists/i.test(message))return json({error:"USER_EXISTS",category:"DOMAIN",retryability:"NON_RETRYABLE"},409);
    throw error;
  }
  return json({id,username,email,organization_id:organizationId,is_admin:Boolean(isAdmin),is_active:true,role:targetRole},201);
}
async function deactivateUser(request:Request,env:Env,session:Session,userId:string){
  requireRole(session,"ALL_FATHER","ORGANIZATION_ADMIN");
  const target=await env.DB.prepare("SELECT id,username,email,supabase_user_id,organization_id,is_admin,is_active,role FROM users WHERE id=? LIMIT 1").bind(userId).first<Record<string,unknown>>();
  if(!target)return json({error:"USER_NOT_FOUND",category:"DOMAIN",retryability:"NON_RETRYABLE"},404);
  const role=roleForUser(target);
  if(session.role==="ALL_FATHER"&&role!=="ORGANIZATION_ADMIN")return json({error:"FORBIDDEN_ROLE",category:"AUTHORITY",retryability:"NON_RETRYABLE"},403);
  if(session.role==="ORGANIZATION_ADMIN"&&(role!=="STAFF"||String(target.organization_id)!==session.organization_id))return json({error:"FORBIDDEN_ROLE",category:"AUTHORITY",retryability:"NON_RETRYABLE"},403);
  if(role==="ALL_FATHER")return json({error:"FORBIDDEN_ROLE",category:"AUTHORITY",retryability:"NON_RETRYABLE"},403);
  // Apply the tenant/role predicate to the write itself, and commit the
  // lifecycle update and audit record together. A concurrent role/tenant
  // change cannot turn the earlier read into an out-of-scope write.
  const timestamp=Date.now();
  const details=JSON.stringify({target:userId,username:target.username,role});
  const update=session.role==="ORGANIZATION_ADMIN"
    ? env.DB.prepare("UPDATE users SET is_active=0,updated_at=? WHERE id=? AND organization_id=? AND role='STAFF' AND is_active=1")
        .bind(timestamp,userId,session.organization_id)
    : env.DB.prepare("UPDATE users SET is_active=0,updated_at=? WHERE id=? AND role='ORGANIZATION_ADMIN' AND is_active=1")
        .bind(timestamp,userId);
  const auditStatement=env.DB.prepare("INSERT INTO audit_log(organization_id,user_id,action,correlation_id,details,created_at) SELECT ?,?,?,?,?,? WHERE changes()=1")
    .bind(session.organization_id,session.sub,"user.deactivate",uuid(),details,timestamp);
  const result=await env.DB.batch([update,auditStatement]);
  if(Number(result[0]?.meta?.changes??0)!==1)
    return json({error:"USER_CHANGED",category:"CONCURRENCY",retryability:"RETRYABLE"},409);
  return json({success:true,id:userId,is_active:false});
}
async function audit(env:Env,session:Session,action:string,targetId:string,details:Json){await env.DB.prepare("INSERT INTO audit_log(organization_id,user_id,action,correlation_id,details,created_at) VALUES(?,?,?,?,?,?)").bind(session.organization_id,session.sub,action,uuid(),JSON.stringify({target:targetId,...details}),Date.now()).run();}

function queryEnvelope(raw:string):Json|null{try{return JSON.parse(new TextDecoder().decode(fromB64u(raw))) as Json;}catch{return null;}}
function safeJson(value:string):unknown{try{return JSON.parse(value);}catch{return value;}}
function isActivePolicyState(value:unknown):boolean{return Boolean(value)&&typeof value==="object"&&!Array.isArray(value)&&(value as Record<string,unknown>).status==="Active";}
function freshnessDto(rows: Array<{version?: unknown; updated_at?: unknown}>){
  let projectionVersion=0,latestUpdated=0;
  for(const row of rows){
    const version=Number(row.version);
    if(Number.isFinite(version))projectionVersion=Math.max(projectionVersion,version);
    const updated=typeof row.updated_at==="number"?row.updated_at:
      typeof row.updated_at==="string"?(Number.isFinite(Number(row.updated_at))?Number(row.updated_at):Date.parse(row.updated_at)):0;
    if(Number.isFinite(updated))latestUpdated=Math.max(latestUpdated,updated);
  }
  return {projection_version:projectionVersion,last_updated_at:new Date(latestUpdated).toISOString(),is_stale:false};
}
function compareQueryValues(left:unknown,right:unknown,sortOrder:"asc"|"desc"):number{
  let result=0;
  if(left==null&&right!=null)result=-1;
  else if(left!=null&&right==null)result=1;
  else if(typeof left==="number"&&typeof right==="number")result=left-right;
  else if(typeof left==="boolean"&&typeof right==="boolean")result=Number(left)-Number(right);
  else if(left!=null&&right!=null){
    const leftDate=typeof left==="string"?Date.parse(left):NaN,rightDate=typeof right==="string"?Date.parse(right):NaN;
    if(Number.isFinite(leftDate)&&Number.isFinite(rightDate))result=leftDate-rightDate;
    else result=String(left).localeCompare(String(right),undefined,{numeric:true,sensitivity:"base"});
  }
  return sortOrder==="asc"?result:-result;
}
async function queryRoute(request:Request,env:Env,session:Session){
  const raw=new URL(request.url).searchParams.get("envelope");const envelope=raw?queryEnvelope(raw):null;
  if(!envelope||typeof envelope.query_id!=="string"||typeof envelope.query_type!=="string"||typeof envelope.organization_id!=="string")return json({error:"INVALID_QUERY_ENVELOPE",category:"DOMAIN",retryability:"NON_RETRYABLE"},400);
  if(envelope.organization_id!==session.organization_id)return json({error:"TENANT_MISMATCH",category:"AUTHORITY",retryability:"NON_RETRYABLE"},403);
  const type=envelope.query_type;
  const aggregateType=OBSERVER_QUERIES[type];
  if(type!=="dashboard.summary"&&!aggregateType)return json({error:"UNKNOWN_QUERY",category:"AUTHORITY",retryability:"NON_RETRYABLE"},403);
  if(!authorizeQuery(session.role,type))return json({error:"FORBIDDEN",category:"AUTHORITY",retryability:"NON_RETRYABLE"},403);
  if(type==="dashboard.summary"){
    const ownOnly=session.role==="STAFF",ownSql=ownOnly?` AND ${staffOwnedPredicateSql()}`:"",ownArgs=ownOnly?staffOwnedBindings(session.sub):[];
    const rows=await env.DB.prepare(`SELECT aggregate_type,version,updated_at FROM aggregates WHERE organization_id=?${ownSql}`)
      .bind(session.organization_id,...ownArgs).all<Record<string,unknown>>();
    const allRows=rows.results??[],counts=new Map<string,number>();
    for(const row of allRows)counts.set(String(row.aggregate_type),(counts.get(String(row.aggregate_type))??0)+1);
    const data=Array.from(counts,([aggregate_type,count])=>({aggregate_type,count}));
    return json({query_id:envelope.query_id,data,has_more:false,next_cursor:null,total_count:data.length,freshness:freshnessDto(allRows)});
  }
  const filters=(envelope.filters&&typeof envelope.filters==="object"&&!Array.isArray(envelope.filters)?envelope.filters:{}) as Json;
  const targetId=typeof filters.target_id==="string"?filters.target_id:typeof envelope.target_id==="string"?envelope.target_id:null;
  const ownOnly=session.role==="STAFF";
  const ownSql=ownOnly?` AND ${staffOwnedPredicateSql()}`:"";
  const ownArgs=ownOnly?staffOwnedBindings(session.sub):[];
  const queryTypeFilter=aggregateType==="*"?"": " AND aggregate_type=?";
  const sql=`SELECT id,aggregate_type,version,lifecycle_epoch,authority_epoch,state,updated_at FROM aggregates WHERE organization_id=?${queryTypeFilter}${ownSql} ORDER BY updated_at DESC`;
  const bindings=aggregateType==="*"?[session.organization_id,...ownArgs]:[session.organization_id,aggregateType,...ownArgs];
  const rows=await env.DB.prepare(sql).bind(...bindings).all<Record<string,unknown>>();
  const allRows=rows.results??[];
  let selected=allRows.filter(row=>!targetId||String(row.id)===targetId);
  const isDetail=type.endsWith(".detail");
  if(isDetail)selected=selected.slice(0,1);
  const normalized=selected.map(row=>{
    const state=typeof row.state==="string"?safeJson(row.state):row.state;
    return {...row,state};
  });
  const requestedSort=typeof envelope.sort_by==="string"&&/^[a-zA-Z_][a-zA-Z0-9_.]*$/.test(envelope.sort_by)?envelope.sort_by:"updated_at";
  const sortOrder=envelope.sort_order==="asc"?"asc":"desc";
  if(!isDetail){
    normalized.sort((a,b)=>{
      const aState=a.state&&typeof a.state==="object"&&!Array.isArray(a.state)?a.state as Json:{};
      const bState=b.state&&typeof b.state==="object"&&!Array.isArray(b.state)?b.state as Json:{};
      const aValue=Object.prototype.hasOwnProperty.call(a,requestedSort)?a[requestedSort]:aState[requestedSort];
      const bValue=Object.prototype.hasOwnProperty.call(b,requestedSort)?b[requestedSort]:bState[requestedSort];
      const compared=compareQueryValues(aValue,bValue,sortOrder);
      return compared||String(a.id).localeCompare(String(b.id));
    });
  }
  const totalCount=normalized.length;
  const parsedLimit=Number(envelope.limit);
  const limit=envelope.limit==null?100:Number.isFinite(parsedLimit)?Math.max(1,Math.min(200,Math.trunc(parsedLimit))):100;
  const cursor=typeof envelope.cursor==="string"&&/^\d+$/.test(envelope.cursor)?Number(envelope.cursor):0;
  const offset=Number.isSafeInteger(cursor)&&cursor>=0?cursor:0;
  const page=isDetail?normalized.slice(0,1):normalized.slice(offset,offset+limit);
  const nextOffset=offset+page.length;
  const hasMore=!isDetail&&nextOffset<totalCount;
  return json({query_id:envelope.query_id,data:page,has_more:hasMore,next_cursor:hasMore?String(nextOffset):null,total_count:totalCount,freshness:freshnessDto(allRows)});
}

async function commandRoute(request:Request,env:Env,session:Session){
  const envelope=await request.json().catch(()=>null) as Json|null;
  if(!envelope||typeof envelope.command_id!=="string"||!envelope.command_id||envelope.command_id.length>128||
     typeof envelope.operation_id!=="string"||!envelope.operation_id||envelope.operation_id.length>128||
     typeof envelope.command_type!=="string"||!envelope.command_type||envelope.command_type.length>128)
    return json({error:"INVALID_COMMAND_ENVELOPE",category:"DOMAIN",retryability:"NON_RETRYABLE"},400);
  const targetValue=envelope.target;
  if(!targetValue||typeof targetValue!=="object"||Array.isArray(targetValue))
    return json({error:"INVALID_COMMAND_TARGET",category:"DOMAIN",retryability:"NON_RETRYABLE"},400);
  const target=targetValue as Json;
  if(typeof target.id!=="string"||!target.id||target.id.length>256||
     typeof target.type!=="string"||!target.type||typeof target.organization_id!=="string")
    return json({error:"INVALID_COMMAND_TARGET",category:"DOMAIN",retryability:"NON_RETRYABLE"},400);
  const targetId=target.id,targetType=target.type;
  if(target.organization_id!==session.organization_id)
    return json({error:"TENANT_MISMATCH",category:"AUTHORITY",retryability:"NON_RETRYABLE"},403);
  const decision=authorizeCommand(session.role,envelope.command_type,targetType);
  if(!decision.allowed)return json({error:decision.reason==="UNKNOWN_COMMAND"?"UNKNOWN_COMMAND":"FORBIDDEN",category:"AUTHORITY",retryability:"NON_RETRYABLE"},403);
  const policy=decision.policy;
  const payloadValue=envelope.payload===undefined?{}:envelope.payload;
  // The Axum contract accepts serde_json::Value, not just JSON objects.
  // Normalize scalar/array values to an empty field map for the Worker's
  // field-level authorization checks; command semantics still reject missing
  // required fields for commands that actually require a structured payload.
  const payload=payloadValue!==null&&typeof payloadValue==="object"&&!Array.isArray(payloadValue)
    ?payloadValue as Json:{};
  // The Worker is not the domain state-machine authority. Clients may not
  // forge lifecycle/tenant/security fields by smuggling them into a payload.
  const forbiddenPayloadKeys=["status","state","lifecycle","lifecycle_state","version","role","is_admin","is_active","organization_id","organizationId","tenant_id","_command_type","_updated_by"];
  if(Object.keys(payload).some(key=>forbiddenPayloadKeys.includes(key)))
    return json({error:"INVALID_COMMAND_PAYLOAD",category:"DOMAIN",retryability:"NON_RETRYABLE"},400);
  const prior=await env.DB.prepare("SELECT result FROM idempotency WHERE operation_id=? LIMIT 1").bind(envelope.operation_id).first<{result:string}>();
  if(prior){
    const priorResult=safeJson(prior.result) as Json;
    const priorEvents=Array.isArray(priorResult.events)?priorResult.events:[];
    const sameActorReplay=priorEvents.some(item=>{
      if(!item||typeof item!=="object")return false;
      const event=item as Json;
      const actor=event.actor&&typeof event.actor==="object"?event.actor as Json:{};
      return event.organization_id===session.organization_id&&event.aggregate_id===targetId&&
        event.aggregate_type===targetType&&event.event_type===envelope.command_type&&
        actor.user_id===session.sub&&JSON.stringify(event.payload??{})===JSON.stringify(payload);
    });
    if(!sameActorReplay)return json({error:"IDEMPOTENCY_CONFLICT",category:"AUTHORITY",retryability:"NON_RETRYABLE"},409);
    return json(priorResult);
  }
  const expected=Number(envelope.expected_version??0);
  const expectedLifecycle=Number(envelope.expected_lifecycle_epoch??0);
  const expectedAuthority=Number(envelope.expected_authority_epoch??0);
  if(!Number.isSafeInteger(expected)||expected<0||!Number.isSafeInteger(expectedLifecycle)||expectedLifecycle<0||
     !Number.isSafeInteger(expectedAuthority)||expectedAuthority<0)
    return json({error:"INVALID_COMMAND_VERSION",category:"DOMAIN",retryability:"NON_RETRYABLE"},400);
  const existing=await env.DB.prepare(
    "SELECT aggregate_type,version,lifecycle_epoch,authority_epoch,state FROM aggregates WHERE id=? AND organization_id=? LIMIT 1"
  ).bind(targetId,session.organization_id).first<{aggregate_type:string;version:number;lifecycle_epoch:number;authority_epoch:number;state:string}>();
  if(existing&&existing.aggregate_type!==targetType)
    return json({error:"TARGET_TYPE_MISMATCH",category:"AUTHORITY",retryability:"NON_RETRYABLE"},403);
  if(policy.effect==="create"&&existing)
    return json({error:"RESOURCE_ALREADY_EXISTS",category:"DOMAIN",retryability:"NON_RETRYABLE"},409);
  if(policy.effect==="update"&&!existing)
    return json({error:"RESOURCE_NOT_FOUND",category:"DOMAIN",retryability:"NON_RETRYABLE"},404);
  if(existing&&(expected!==Number(existing.version)||expectedLifecycle!==Number(existing.lifecycle_epoch)||expectedAuthority!==Number(existing.authority_epoch)))
    return json({success:false,operation_id:envelope.operation_id,events:[],error:{code:"VERSION_CONFLICT",category:"CONCURRENCY",retryability:"RETRYABLE",safe_details:{expected,actual:existing.version},correlation_id:String(envelope.correlation_id??envelope.operation_id)}},409);
  const previousState=existing?safeJson(existing.state):null;
  const semantic=validateCommandSemantics(envelope.command_type,previousState,payload,session.sub);
  if(!semantic.allowed){
    const status=semantic.reason==="INVALID_COMMAND_PAYLOAD"?400:
      semantic.reason==="INVALID_STATE_TRANSITION"||semantic.reason==="RESOURCE_ALREADY_EXISTS"||semantic.reason==="RESOURCE_STATE_UNVERIFIED"?409:403;
    return json({error:semantic.reason,category:status===403?"AUTHORITY":"DOMAIN",retryability:"NON_RETRYABLE"},status);
  }
  if(policy.ownerRequiredOnCreate&&policy.effect==="create"&&payloadOwnerIds(payload).length===0)
    return json({error:"RESOURCE_OWNER_REQUIRED",category:"DOMAIN",retryability:"NON_RETRYABLE"},400);
  if(policy.staffMustOwn&&session.role==="STAFF"){
    if(policy.effect==="create"){
      if(!resourceOwnedBy(payload,session.sub))return json({error:"RESOURCE_NOT_OWNED",category:"AUTHORITY",retryability:"NON_RETRYABLE"},403);
    }else if(!existing||!resourceOwnedBy(safeJson(existing.state),session.sub))
      return json({error:"RESOURCE_NOT_OWNED",category:"AUTHORITY",retryability:"NON_RETRYABLE"},403);
    if(payloadOwnerIds(payload).some(ownerId=>ownerId!==session.sub))
      return json({error:"RESOURCE_NOT_OWNED",category:"AUTHORITY",retryability:"NON_RETRYABLE"},403);
  }
  for(const ownerId of payloadOwnerIds(payload)){
    const owner=await env.DB.prepare("SELECT id FROM users WHERE id=? AND organization_id=? AND is_active=1 LIMIT 1")
      .bind(ownerId,session.organization_id).first();
    if(!owner)return json({error:"INVALID_RESOURCE_OWNER",category:"AUTHORITY",retryability:"NON_RETRYABLE"},403);
  }
  // Resolve cross-aggregate references against D1 with tenant and aggregate
  // type predicates; a client-supplied identifier is never sufficient proof.
  if(payload.mission_id!==undefined){
    const missionId=resourceIdFromReference(payload.mission_id);
    if(!missionId)return json({error:"INVALID_MISSION_REFERENCE",category:"AUTHORITY",retryability:"NON_RETRYABLE"},403);
    const mission=await env.DB.prepare("SELECT id FROM aggregates WHERE id=? AND organization_id=? AND aggregate_type='mission' LIMIT 1")
      .bind(missionId,session.organization_id).first();
    if(!mission)return json({error:"INVALID_MISSION_REFERENCE",category:"AUTHORITY",retryability:"NON_RETRYABLE"},403);
  }
  if(typeof payload.blocking_ref==="string"&&payload.blocking_ref){
    const blockerId=resourceIdFromReference(payload.blocking_ref);
    if(!blockerId||blockerId===targetId)return json({error:"INVALID_BLOCKING_REFERENCE",category:"DOMAIN",retryability:"NON_RETRYABLE"},400);
    const blocker=await env.DB.prepare("SELECT id FROM aggregates WHERE id=? AND organization_id=? AND aggregate_type='task' LIMIT 1")
      .bind(blockerId,session.organization_id).first();
    if(!blocker)return json({error:"INVALID_BLOCKING_REFERENCE",category:"AUTHORITY",retryability:"NON_RETRYABLE"},403);
  }
  if(envelope.command_type==="AddDependency"){
    const dependency=payload.dependency&&typeof payload.dependency==="object"&&!Array.isArray(payload.dependency)?payload.dependency as Json:null;
    if(!dependency||Object.keys(dependency).some(key=>key!=="task_id"&&key!=="dependency_type")||
       !["FinishToStart","StartToStart","FinishToFinish"].includes(String(dependency.dependency_type)))
      return json({error:"INVALID_DEPENDENCY",category:"DOMAIN",retryability:"NON_RETRYABLE"},400);
    const dependencyId=resourceIdFromReference(dependency.task_id);
    if(!dependencyId||dependencyId===targetId)return json({error:"INVALID_DEPENDENCY",category:"DOMAIN",retryability:"NON_RETRYABLE"},400);
    const dependencyTask=await env.DB.prepare("SELECT id FROM aggregates WHERE id=? AND organization_id=? AND aggregate_type='task' LIMIT 1")
      .bind(dependencyId,session.organization_id).first();
    if(!dependencyTask)return json({error:"INVALID_DEPENDENCY",category:"AUTHORITY",retryability:"NON_RETRYABLE"},403);
  }
  if(envelope.command_type==="ApplyLegalHold"){
    const targetRef=payload.target&&typeof payload.target==="object"&&!Array.isArray(payload.target)?payload.target as Json:null;
    if(!targetRef||Object.keys(targetRef).some(key=>key!=="target_id"&&key!=="target_type")||
       typeof targetRef.target_type!=="string")
      return json({error:"INVALID_HOLD_TARGET",category:"DOMAIN",retryability:"NON_RETRYABLE"},400);
    const holdTargetId=resourceIdFromReference(targetRef.target_id);
    if(!holdTargetId)return json({error:"INVALID_HOLD_TARGET",category:"AUTHORITY",retryability:"NON_RETRYABLE"},403);
    const held=targetRef.target_type==="file_asset"
      ? await env.DB.prepare("SELECT content_hash AS id FROM file_assets WHERE content_hash=? AND organization_id=? LIMIT 1")
          .bind(holdTargetId,session.organization_id).first()
      : await env.DB.prepare("SELECT id FROM aggregates WHERE id=? AND organization_id=? AND aggregate_type=? LIMIT 1")
          .bind(holdTargetId,session.organization_id,targetRef.target_type).first();
    if(!held)return json({error:"INVALID_HOLD_TARGET",category:"AUTHORITY",retryability:"NON_RETRYABLE"},403);
  }
  if(envelope.command_type==="CreatePolicy"){
    const scope=payload.scope as Json;
    const scopeKind=Object.keys(scope)[0];
    const scopeId=resourceIdFromReference(scope[scopeKind]);
    if(!scopeId)return json({error:"INVALID_POLICY_SCOPE",category:"AUTHORITY",retryability:"NON_RETRYABLE"},403);
    if(scopeKind==="Organization"){
      if(scopeId!==session.organization_id)return json({error:"TENANT_MISMATCH",category:"AUTHORITY",retryability:"NON_RETRYABLE"},403);
    }else{
      const conversation=await env.DB.prepare("SELECT id FROM aggregates WHERE id=? AND organization_id=? AND aggregate_type='conversation' LIMIT 1")
        .bind(scopeId,session.organization_id).first();
      if(!conversation)return json({error:"INVALID_POLICY_SCOPE",category:"AUTHORITY",retryability:"NON_RETRYABLE"},403);
    }
  }
  for(const evidenceField of ["evidence","closure_evidence","completion_evidence"]){
    const evidence=payload[evidenceField];
    if(evidence!==undefined){
      if(!Array.isArray(evidence))return json({error:"INVALID_EVIDENCE_REFERENCE",category:"AUTHORITY",retryability:"NON_RETRYABLE"},403);
      for(const value of evidence){
        const evidenceId=resourceIdFromReference(value);
        if(!evidenceId)return json({error:"INVALID_EVIDENCE_REFERENCE",category:"AUTHORITY",retryability:"NON_RETRYABLE"},403);
        const aggregate=await env.DB.prepare("SELECT id FROM aggregates WHERE id=? AND organization_id=? LIMIT 1")
          .bind(evidenceId,session.organization_id).first();
        const file=aggregate?null:await env.DB.prepare("SELECT content_hash FROM file_assets WHERE content_hash=? AND organization_id=? LIMIT 1")
          .bind(evidenceId,session.organization_id).first();
        if(!aggregate&&!file)return json({error:"INVALID_EVIDENCE_REFERENCE",category:"AUTHORITY",retryability:"NON_RETRYABLE"},403);
      }
    }
  }
  if(typeof payload.authorizing_policy==="string"&&payload.authorizing_policy){
    const policyId=resourceIdFromReference(payload.authorizing_policy);
    if(!policyId)return json({error:"INVALID_AUTHORIZING_POLICY",category:"AUTHORITY",retryability:"NON_RETRYABLE"},403);
    const authorizingPolicy=await env.DB.prepare("SELECT id,state FROM aggregates WHERE id=? AND organization_id=? AND aggregate_type='policy' LIMIT 1")
      .bind(policyId,session.organization_id).first<{id:string;state:string}>();
    if(!authorizingPolicy||!isActivePolicyState(safeJson(authorizingPolicy.state)))
      return json({error:"INVALID_AUTHORIZING_POLICY",category:"AUTHORITY",retryability:"NON_RETRYABLE"},403);
  }
  const previous=previousState;
  const merged={
    ...((previous&&typeof previous==="object"&&!Array.isArray(previous)?previous:{}) as Json),
    ...(policy.effect==="create"?payload:{}),
    ...(semantic.statePatch??{}),
    status:semantic.nextStatus,
    _command_type:envelope.command_type,
    _updated_by:session.sub,
  };
  const nextVersion=(existing?.version??0)+1;
  const currentStatus=previous&&typeof previous==="object"&&!Array.isArray(previous)?(previous as Json).status:null;
  const nextLifecycle=existing&&semantic.nextStatus!==currentStatus?expectedLifecycle+1:expectedLifecycle;
  const authorityChangingActions=["AssignOwner","PublishPolicyVersion","RetirePolicy","approval.Approve","approval.Reject","ApplyLegalHold","ReleaseLegalHold"];
  const nextAuthority=existing&&authorityChangingActions.includes(envelope.command_type)?expectedAuthority+1:expectedAuthority;
  const event={event_id:uuid(),aggregate_id:targetId,aggregate_type:targetType,event_type:envelope.command_type,aggregate_version:nextVersion,organization_id:session.organization_id,payload,occurred_at:new Date().toISOString(),actor:{user_id:session.sub,organization_id:session.organization_id}};
  const result={success:true,operation_id:envelope.operation_id,new_version:nextVersion,new_lifecycle_epoch:nextLifecycle,new_authority_epoch:nextAuthority,events:[event],result:{id:targetId,type:targetType,state:merged}};
  // D1 batch is atomic. Conditional event/audit/idempotency inserts mean a
  // failed optimistic insert/update cannot leave partial side effects.
  const aggregateStatement=policy.effect==="create"
    ? env.DB.prepare("INSERT OR IGNORE INTO aggregates(id,aggregate_type,version,lifecycle_epoch,authority_epoch,state,updated_at,organization_id) VALUES(?,?,?,?,?,?,?,?)").bind(targetId,targetType,nextVersion,nextLifecycle,nextAuthority,JSON.stringify(merged),Date.now(),session.organization_id)
    : env.DB.prepare("UPDATE aggregates SET aggregate_type=?,version=?,lifecycle_epoch=?,authority_epoch=?,state=?,updated_at=? WHERE id=? AND organization_id=? AND aggregate_type=? AND version=? AND lifecycle_epoch=? AND authority_epoch=?").bind(targetType,nextVersion,nextLifecycle,nextAuthority,JSON.stringify(merged),Date.now(),targetId,session.organization_id,targetType,expected,expectedLifecycle,expectedAuthority);
  const eventStatement=env.DB.prepare("INSERT INTO domain_events(event_id,aggregate_id,aggregate_version,event_type,payload,occurred_at,vector_clock,operation_id,correlation_id,actor,organization_id) SELECT ?,?,?,?,?,?,?,?,?,?,? WHERE changes()=1").bind(event.event_id,targetId,nextVersion,envelope.command_type,JSON.stringify(payload),Date.now(),"{}",envelope.operation_id,String(envelope.correlation_id??envelope.operation_id),JSON.stringify(event.actor),session.organization_id);
  const idemStatement=env.DB.prepare("INSERT INTO idempotency(operation_id,result,created_at) SELECT ?,?,? WHERE EXISTS (SELECT 1 FROM domain_events WHERE event_id=? AND organization_id=?)").bind(envelope.operation_id,JSON.stringify(result),Date.now(),event.event_id,session.organization_id);
  const auditStatement=env.DB.prepare("INSERT INTO audit_log(organization_id,user_id,action,correlation_id,details,created_at) SELECT ?,?,?,?,?,? WHERE EXISTS (SELECT 1 FROM domain_events WHERE event_id=? AND organization_id=?)").bind(session.organization_id,session.sub,envelope.command_type,String(envelope.correlation_id??envelope.operation_id),JSON.stringify({target:targetId}),Date.now(),event.event_id,session.organization_id);
  const batch=await env.DB.batch([aggregateStatement,eventStatement,idemStatement,auditStatement]);
  if(Number(batch[0]?.meta?.changes??0)!==1)
    return json({success:false,operation_id:envelope.operation_id,events:[],error:{code:"VERSION_CONFLICT",category:"CONCURRENCY",retryability:"RETRYABLE",safe_details:{expected},correlation_id:String(envelope.correlation_id??envelope.operation_id)}},409);
  return json(result);
}

const MOBILE_ACCESS_CLASSES = ["top_level_manager", "senior_manager", "team_leader", "supervisor", "staff"] as const;
type MobileAccessClass = (typeof MOBILE_ACCESS_CLASSES)[number];
interface MobileAccessState {
  allowed_classes: MobileAccessClass[];
  version: number;
  updated_at: number | null;
  enforcement_status: "disabled_untrusted_client_type";
}
function normalizeMobileClasses(value: unknown): MobileAccessClass[] | null {
  if (!Array.isArray(value) || value.length > MOBILE_ACCESS_CLASSES.length) return null;
  const requested = value as unknown[];
  if (requested.some((item) => typeof item !== "string" ||
    !MOBILE_ACCESS_CLASSES.includes(item as MobileAccessClass)) ||
    new Set(requested).size !== requested.length) return null;
  const selected = new Set(requested as MobileAccessClass[]);
  return MOBILE_ACCESS_CLASSES.filter((item) => selected.has(item));
}
function validClientType(body: Json | null): string | null {
  if (!body || body.client_type === undefined) return "web";
  if (typeof body.client_type !== "string" ||
      !["mobile", "mobile_observer", "desktop", "admin", "web"].includes(body.client_type)) return null;
  return body.client_type;
}
async function readMobileAccess(env: Env, organizationId: string): Promise<MobileAccessState | null> {
  const row = await env.DB.prepare(
    "SELECT organization_id,allowed_classes,version,updated_at FROM mobile_access_settings WHERE organization_id=? LIMIT 1"
  ).bind(organizationId).first<Record<string, unknown>>();
  if (!row) return { allowed_classes: [], version: 0, updated_at: null, enforcement_status: "disabled_untrusted_client_type" };
  let raw: unknown;
  try { raw = JSON.parse(String(row.allowed_classes)); } catch { return null; }
  const allowed = normalizeMobileClasses(raw);
  const version = Number(row.version);
  const updatedAt = Number(row.updated_at);
  if (!allowed || !Number.isSafeInteger(version) || version < 1 ||
      !Number.isSafeInteger(updatedAt) || updatedAt < 0) return null;
  return { allowed_classes: allowed, version, updated_at: updatedAt, enforcement_status: "disabled_untrusted_client_type" };
}
async function getMobileAccess(env: Env, session: Session) {
  requireRole(session, "ALL_FATHER", "ORGANIZATION_ADMIN");
  try {
    const state = await readMobileAccess(env, session.organization_id);
    if (!state) return json({ error: "MOBILE_ACCESS_CONFIG_INVALID", category: "INFRASTRUCTURE", retryability: "NON_RETRYABLE" }, 503);
    return json(state);
  } catch {
    return json({ error: "MOBILE_ACCESS_STORAGE_UNAVAILABLE", category: "INFRASTRUCTURE", retryability: "TRANSIENT" }, 503);
  }
}
async function putMobileAccess(request: Request, env: Env, session: Session) {
  requireRole(session, "ORGANIZATION_ADMIN");
  const body = await request.json().catch(() => null) as Json | null;
  if (!body || Array.isArray(body) || Object.keys(body).some((key) => !["allowed_classes", "expected_version"].includes(key))) {
    return json({ error: "INVALID_MOBILE_ACCESS_CONFIG", category: "DOMAIN", retryability: "NON_RETRYABLE" }, 400);
  }
  const allowed = normalizeMobileClasses(body.allowed_classes);
  if (!allowed) return json({ error: "INVALID_MOBILE_ACCESS_CONFIG", category: "DOMAIN", retryability: "NON_RETRYABLE" }, 400);
  const suppliedVersion = body.expected_version;
  if (suppliedVersion !== undefined && (!Number.isSafeInteger(suppliedVersion) || Number(suppliedVersion) < 0)) {
    return json({ error: "INVALID_EXPECTED_VERSION", category: "DOMAIN", retryability: "NON_RETRYABLE" }, 400);
  }
  try {
    const current = await readMobileAccess(env, session.organization_id);
    if (!current) return json({ error: "MOBILE_ACCESS_CONFIG_INVALID", category: "INFRASTRUCTURE", retryability: "NON_RETRYABLE" }, 503);
    const expectedVersion = suppliedVersion === undefined ? current.version : Number(suppliedVersion);
    if (expectedVersion !== current.version) {
      return json({ error: "VERSION_CONFLICT", category: "CONCURRENCY", retryability: "RETRYABLE", expected_version: current.version }, 409);
    }
    const operationId = uuid();
    const timestamp = Date.now();
    const nextVersion = current.version + 1;
    const save = env.DB.prepare(
      "INSERT INTO mobile_access_settings(organization_id,allowed_classes,version,updated_by,updated_at,last_operation_id) VALUES(?,?,1,?,?,?) " +
      "ON CONFLICT(organization_id) DO UPDATE SET allowed_classes=excluded.allowed_classes,version=mobile_access_settings.version+1,updated_by=excluded.updated_by,updated_at=excluded.updated_at,last_operation_id=excluded.last_operation_id " +
      "WHERE mobile_access_settings.version=?"
    ).bind(session.organization_id, JSON.stringify(allowed), session.sub, timestamp, operationId, expectedVersion);
    const audit = env.DB.prepare(
      "INSERT INTO audit_log(organization_id,user_id,action,correlation_id,details,created_at) " +
      "SELECT ?,?,?,?,?,? WHERE EXISTS (SELECT 1 FROM mobile_access_settings WHERE organization_id=? AND last_operation_id=? AND version=?)"
    ).bind(session.organization_id, session.sub, "mobile_access.update", operationId,
      JSON.stringify({ version: nextVersion, allowed_classes: allowed }), timestamp,
      session.organization_id, operationId, nextVersion);
    const batch = await env.DB.batch([save, audit]);
    if (Number(batch[0]?.meta?.changes ?? 0) !== 1) {
      const latest = await readMobileAccess(env, session.organization_id);
      return json({ error: "VERSION_CONFLICT", category: "CONCURRENCY", retryability: "RETRYABLE",
        expected_version: latest?.version ?? current.version }, 409);
    }
    if (Number(batch[1]?.meta?.changes ?? 0) !== 1) throw new Error("MOBILE_ACCESS_AUDIT_FAILED");
    const persisted = await readMobileAccess(env, session.organization_id);
    if (!persisted || persisted.version !== nextVersion || JSON.stringify(persisted.allowed_classes) !== JSON.stringify(allowed)) {
      return json({ error: "MOBILE_ACCESS_PERSISTENCE_UNVERIFIED", category: "INFRASTRUCTURE", retryability: "TRANSIENT" }, 503);
    }
    return json(persisted);
  } catch (error) {
    if (error instanceof Error && error.message === "MOBILE_ACCESS_AUDIT_FAILED") {
      return json({ error: "MOBILE_ACCESS_AUDIT_FAILED", category: "INFRASTRUCTURE", retryability: "TRANSIENT" }, 503);
    }
    return json({ error: "MOBILE_ACCESS_STORAGE_UNAVAILABLE", category: "INFRASTRUCTURE", retryability: "TRANSIENT" }, 503);
  }
}
async function createAdminAggregate(
  request: Request, env: Env, session: Session,
  commandType: "CreatePolicy" | "ApplyLegalHold", targetType: "policy" | "legal_hold",
  payload: Json, responseField: "policy_id" | "legal_hold_id", representationField: "policy" | "legal_hold"
) {
  const key = request.headers.get("idempotency-key");
  if (key !== null && (!key.trim() || key.length > 128 || /[\r\n]/.test(key))) {
    return json({ error: "INVALID_IDEMPOTENCY_KEY", category: "DOMAIN", retryability: "NON_RETRYABLE" }, 400);
  }
  const operationId = key
    ? "rest-" + (await sha256(session.organization_id + "\n" + session.sub + "\n" + new URL(request.url).pathname + "\n" + key))
    : uuid();
  if (key) {
    const prior = await env.DB.prepare("SELECT result FROM idempotency WHERE operation_id=? LIMIT 1")
      .bind(operationId).first<{ result: string }>();
    if (prior) {
      const priorResult = safeJson(prior.result) as Json;
      const events = Array.isArray(priorResult.events) ? priorResult.events : [];
      const event = events[0] as Json | undefined;
      const actor = event?.actor && typeof event.actor === "object" ? event.actor as Json : {};
      const sameRequest = event?.organization_id === session.organization_id &&
        event?.aggregate_type === targetType && event?.event_type === commandType &&
        actor.user_id === session.sub && JSON.stringify(event?.payload ?? {}) === JSON.stringify(payload);
      if (!sameRequest) return json({ error: "IDEMPOTENCY_CONFLICT", category: "AUTHORITY", retryability: "NON_RETRYABLE" }, 409);
      const priorEntity = priorResult.result && typeof priorResult.result === "object" ? priorResult.result as Json : {};
      return json({
        [responseField]: String(priorEntity.id ?? ""),
        [representationField]: priorEntity.state ?? null,
        version: Number(priorResult.new_version ?? 1),
      }, 201);
    }
  }
  const targetId = uuid();
  const envelope = {
    command_id: uuid(), operation_id: operationId, command_type: commandType,
    target: { id: targetId, type: targetType, organization_id: session.organization_id },
    expected_version: 0, expected_lifecycle_epoch: 0, expected_authority_epoch: 0, payload,
  };
  const response = await commandRoute(new Request(request.url, {
    method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify(envelope),
  }), env, session);
  const result = await response.json().catch(() => ({})) as Json;
  if (!response.ok) return json(result, response.status);
  const entity = result.result && typeof result.result === "object" ? result.result as Json : {};
  return json({
    [responseField]: String(entity.id ?? targetId),
    [representationField]: entity.state ?? null,
    version: Number(result.new_version ?? 1),
  }, 201);
}
async function postAdminPolicy(request: Request, env: Env, session: Session) {
  requireRole(session, "ORGANIZATION_ADMIN");
  const body = await request.json().catch(() => null) as Json | null;
  if (!body || Array.isArray(body) || Object.keys(body).some((key) => key !== "name") ||
      typeof body.name !== "string" || !body.name.trim() || body.name.trim().length > 200) {
    return json({ error: "INVALID_POLICY", category: "DOMAIN", retryability: "NON_RETRYABLE" }, 400);
  }
  const payload: Json = { name: body.name.trim(), scope: { Organization: session.organization_id } };
  return createAdminAggregate(request, env, session, "CreatePolicy", "policy", payload, "policy_id", "policy");
}
const HOLD_TARGET_TYPES = new Set(["task", "mission", "approval", "policy", "todo_list", "target_list", "staff_loan", "notification", "report"]);
function looksLikeUuid(value: string): boolean {
  return /^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(value);
}
async function postAdminLegalHold(request: Request, env: Env, session: Session) {
  requireRole(session, "ORGANIZATION_ADMIN");
  const body = await request.json().catch(() => null) as Json | null;
  if (!body || Array.isArray(body) ||
      Object.keys(body).some((key) => !["target_id", "target_type", "reason"].includes(key)) ||
      typeof body.target_id !== "string" || !looksLikeUuid(body.target_id) ||
      typeof body.target_type !== "string" || !HOLD_TARGET_TYPES.has(body.target_type) ||
      typeof body.reason !== "string" || !body.reason.trim() || body.reason.trim().length > 2000) {
    return json({ error: "INVALID_LEGAL_HOLD", category: "DOMAIN", retryability: "NON_RETRYABLE" }, 400);
  }
  const targetId = body.target_id.toLowerCase();
  let target: Record<string, unknown> | null = null;
  try {
    target = await env.DB.prepare("SELECT id FROM aggregates WHERE id=? AND organization_id=? AND aggregate_type=? LIMIT 1")
      .bind(targetId, session.organization_id, body.target_type).first<Record<string, unknown>>();
  } catch {
    return json({ error: "RESOURCE_LOOKUP_UNAVAILABLE", category: "INFRASTRUCTURE", retryability: "TRANSIENT" }, 503);
  }
  if (!target) return json({ error: "LEGAL_HOLD_TARGET_NOT_FOUND", category: "DOMAIN", retryability: "NON_RETRYABLE" }, 404);
  const payload: Json = {
    target: { target_id: targetId, target_type: body.target_type },
    reason: body.reason.trim(),
  };
  return createAdminAggregate(request, env, session, "ApplyLegalHold", "legal_hold", payload, "legal_hold_id", "legal_hold");
}
async function listUsers(env: Env, session: Session) {
  // Minimal active identity picker, always scoped to the authenticated tenant.
  const rows = await env.DB.prepare(
    "SELECT id,username FROM users WHERE organization_id=? AND is_active=1 ORDER BY username LIMIT 500"
  ).bind(session.organization_id).all<Record<string, unknown>>();
  return json((rows.results ?? []).map((row) => ({
    id: String(row.id),
    username: String(row.username),
  })));
}

async function listAdminUsers(env: Env, session: Session) {
  // The organization Users screen is for organization administrators only.
  // All-Father manages organizations and their Admins through dedicated routes;
  // this endpoint must never become a cross-tenant directory.
  requireRole(session, "ORGANIZATION_ADMIN");
  const rows = await env.DB.prepare(
    "SELECT id,username,organization_id,is_admin,is_manager,class,parent_user_id,is_active FROM users WHERE organization_id=? ORDER BY username LIMIT 500"
  ).bind(session.organization_id).all<Record<string, unknown>>();
  // is_manager is retained as a legacy UI field; the canonical Worker schema
  // models authority with role and operational classification with class.
  return json((rows.results ?? []).map((row) => ({
    id: String(row.id),
    username: String(row.username),
    organization_id: String(row.organization_id),
    is_admin: row.is_admin === true || row.is_admin === 1 || row.is_admin === "1",
    is_manager: row.is_manager === true || row.is_manager === 1 || row.is_manager === "1",
    class: typeof row.class === "string" ? row.class : null,
    parent_user_id: typeof row.parent_user_id === "string" ? row.parent_user_id : null,
    is_active: row.is_active === true || row.is_active === 1 || row.is_active === "1",
  })));
}

async function assignUserClass(request: Request, env: Env, session: Session, userId: string) {
  requireRole(session, "ORGANIZATION_ADMIN");
  const body = await request.json().catch(() => null) as Json | null;
  if (!body || Array.isArray(body) || Object.keys(body).length !== 1 || !Object.prototype.hasOwnProperty.call(body, "class")) {
    return json({ error: "INVALID_USER_CLASS", category: "DOMAIN", retryability: "NON_RETRYABLE" }, 400);
  }
  const value = body.class;
  if (value !== null && (typeof value !== "string" || !MOBILE_ACCESS_CLASSES.includes(value as MobileAccessClass))) {
    return json({ error: "INVALID_USER_CLASS", category: "DOMAIN", retryability: "NON_RETRYABLE" }, 400);
  }
  const timestamp = Date.now();
  const operationId = uuid();
  try {
    const update = env.DB.prepare("UPDATE users SET class=?,updated_at=? WHERE id=? AND organization_id=? AND is_active=1")
      .bind(value, timestamp, userId, session.organization_id);
    const audit = env.DB.prepare(
      "INSERT INTO audit_log(organization_id,user_id,action,correlation_id,details,created_at) SELECT ?,?,?,?,?,? WHERE changes()=1"
    ).bind(session.organization_id, session.sub, "user.class.update", operationId,
      JSON.stringify({ target: userId, class: value }), timestamp);
    const batch = await env.DB.batch([update, audit]);
    if (Number(batch[0]?.meta?.changes ?? 0) !== 1) return json({ error: "USER_NOT_FOUND", category: "DOMAIN", retryability: "NON_RETRYABLE" }, 404);
    if (Number(batch[1]?.meta?.changes ?? 0) !== 1) throw new Error("USER_CLASS_AUDIT_FAILED");
    return json({ success: true, class: value });
  } catch {
    return json({ error: "USER_CLASS_STORAGE_UNAVAILABLE", category: "INFRASTRUCTURE", retryability: "TRANSIENT" }, 503);
  }
}


async function setUserActive(env: Env, session: Session, userId: string, active: boolean) {
  requireRole(session, "ORGANIZATION_ADMIN");
  if (userId === session.sub && !active) return json({ error: "CANNOT_DEACTIVATE_SELF", category: "DOMAIN", retryability: "NON_RETRYABLE" }, 400);
  const timestamp = Date.now(), operationId = uuid();
  try {
    const batch = await env.DB.batch([
      env.DB.prepare("UPDATE users SET is_active=?,updated_at=? WHERE id=? AND organization_id=? AND role='STAFF' AND id<>'__allfather__'").bind(active ? 1 : 0, timestamp, userId, session.organization_id),
      env.DB.prepare("INSERT INTO audit_log(organization_id,user_id,action,correlation_id,details,created_at) SELECT ?,?,?,?,?,? WHERE changes()=1").bind(session.organization_id, session.sub, active ? "user.activate" : "user.deactivate", operationId, JSON.stringify({ target: userId }), timestamp),
    ]);
    if (Number(batch[0]?.meta?.changes ?? 0) !== 1) return json({ error: "USER_NOT_FOUND", category: "DOMAIN", retryability: "NON_RETRYABLE" }, 404);
    if (Number(batch[1]?.meta?.changes ?? 0) !== 1) throw new Error("USER_AUDIT_FAILED");
    if (!active) await env.DB.prepare("INSERT INTO user_session_revocations(user_id,revoked_before_ms,updated_at) VALUES(?,?,?) ON CONFLICT(user_id) DO UPDATE SET revoked_before_ms=excluded.revoked_before_ms,updated_at=excluded.updated_at").bind(userId, Date.now(), timestamp).run();
    return json({ success: true });
  } catch {
    return json({ error: "USER_UPDATE_UNAVAILABLE", category: "INFRASTRUCTURE", retryability: "TRANSIENT" }, 503);
  }
}
async function setUserPassword(request: Request, env: Env, session: Session, userId: string) {
  requireRole(session, "ORGANIZATION_ADMIN");
  const body = await request.json().catch(() => null) as Json | null;
  if (!body || Object.keys(body).length !== 1 || typeof body.password !== "string" || body.password.length < 12 || body.password.length > 1024) return json({ error: "WEAK_PASSWORD", message: "Password must contain at least 12 characters.", category: "DOMAIN", retryability: "NON_RETRYABLE" }, 400);
  const target = await env.DB.prepare("SELECT id,username FROM users WHERE id=? AND organization_id=? AND role='STAFF' LIMIT 1").bind(userId, session.organization_id).first<{id:string;username:string}>();
  if (!target || userId === "__allfather__" || target.username.toLowerCase() === "allfather") return json({ error: target ? "RESERVED_IDENTITY" : "USER_NOT_FOUND", category: target ? "AUTHORITY" : "DOMAIN", retryability: "NON_RETRYABLE" }, target ? 403 : 404);
  try {
    const hash = await hashPassword(body.password), timestamp = Date.now(), operationId = uuid();
    const batch = await env.DB.batch([
      env.DB.prepare("UPDATE users SET password_hash=?,updated_at=? WHERE id=? AND organization_id=? AND role='STAFF'").bind(hash, timestamp, userId, session.organization_id),
      env.DB.prepare("INSERT INTO user_session_revocations(user_id,revoked_before_ms,updated_at) VALUES(?,?,?) ON CONFLICT(user_id) DO UPDATE SET revoked_before_ms=excluded.revoked_before_ms,updated_at=excluded.updated_at").bind(userId, timestamp, timestamp),
      env.DB.prepare("INSERT INTO audit_log(organization_id,user_id,action,correlation_id,details,created_at) VALUES(?,?,?,?,?,?)").bind(session.organization_id, session.sub, "user.password.reset", operationId, JSON.stringify({ target: userId }), timestamp),
    ]);
    if (Number(batch[0]?.meta?.changes ?? 0) !== 1) return json({ error: "USER_NOT_FOUND", category: "DOMAIN", retryability: "NON_RETRYABLE" }, 404);
    return json({ success: true });
  } catch {
    return json({ error: "USER_PASSWORD_UPDATE_UNAVAILABLE", category: "INFRASTRUCTURE", retryability: "TRANSIENT" }, 503);
  }
}
async function setUserManager(request: Request, env: Env, session: Session, userId: string) {
  requireRole(session, "ORGANIZATION_ADMIN");
  const body = await request.json().catch(() => null) as Json | null;
  if (!body || Object.keys(body).length !== 1 || typeof body.is_manager !== "boolean") return json({ error: "INVALID_MANAGER_FLAG", category: "DOMAIN", retryability: "NON_RETRYABLE" }, 400);
  const timestamp = Date.now(), operationId = uuid();
  try {
    const batch = await env.DB.batch([
      env.DB.prepare("UPDATE users SET is_manager=?,updated_at=? WHERE id=? AND organization_id=? AND role='STAFF' AND id<>'__allfather__'").bind(body.is_manager ? 1 : 0, timestamp, userId, session.organization_id),
      env.DB.prepare("INSERT INTO audit_log(organization_id,user_id,action,correlation_id,details,created_at) SELECT ?,?,?,?,?,? WHERE changes()=1").bind(session.organization_id, session.sub, "user.manager.update", operationId, JSON.stringify({ target: userId, is_manager: body.is_manager }), timestamp),
    ]);
    if (Number(batch[0]?.meta?.changes ?? 0) !== 1) return json({ error: "USER_NOT_FOUND", category: "DOMAIN", retryability: "NON_RETRYABLE" }, 404);
    if (Number(batch[1]?.meta?.changes ?? 0) !== 1) throw new Error("USER_MANAGER_AUDIT_FAILED");
    return json({ success: true, is_manager: body.is_manager });
  } catch {
    return json({ error: "USER_MANAGER_UPDATE_UNAVAILABLE", category: "INFRASTRUCTURE", retryability: "TRANSIENT" }, 503);
  }
}
async function setUserParent(request: Request, env: Env, session: Session, userId: string) {
  requireRole(session, "ORGANIZATION_ADMIN");
  const body = await request.json().catch(() => null) as Json | null;
  if (!body || Object.keys(body).length !== 1 || !Object.prototype.hasOwnProperty.call(body, "parent_user_id") || (body.parent_user_id !== null && typeof body.parent_user_id !== "string")) return json({ error: "INVALID_PARENT", category: "DOMAIN", retryability: "NON_RETRYABLE" }, 400);
  const parentId = typeof body.parent_user_id === "string" ? body.parent_user_id.trim() : null;
  if (parentId === userId) return json({ error: "PARENT_CYCLE", category: "DOMAIN", retryability: "NON_RETRYABLE" }, 400);
  const target = await env.DB.prepare("SELECT id FROM users WHERE id=? AND organization_id=? AND role='STAFF' AND id<>'__allfather__' LIMIT 1").bind(userId, session.organization_id).first();
  if (!target) return json({ error: "USER_NOT_FOUND", category: "DOMAIN", retryability: "NON_RETRYABLE" }, 404);
  if (parentId) {
    const parent = await env.DB.prepare("SELECT id FROM users WHERE id=? AND organization_id=? AND is_active=1 AND id<>'__allfather__' LIMIT 1").bind(parentId, session.organization_id).first();
    if (!parent) return json({ error: "PARENT_USER_NOT_FOUND", category: "DOMAIN", retryability: "NON_RETRYABLE" }, 400);
    const cycle = await env.DB.prepare("WITH RECURSIVE descendants(id) AS (SELECT id FROM users WHERE parent_user_id=? AND organization_id=? UNION ALL SELECT u.id FROM users u JOIN descendants d ON u.parent_user_id=d.id WHERE u.organization_id=?) SELECT 1 AS found FROM descendants WHERE id=? LIMIT 1").bind(userId, session.organization_id, session.organization_id, parentId).first();
    if (cycle) return json({ error: "PARENT_CYCLE", category: "DOMAIN", retryability: "NON_RETRYABLE" }, 400);
  }
  const timestamp = Date.now(), operationId = uuid();
  try {
    const batch = await env.DB.batch([
      env.DB.prepare("UPDATE users SET parent_user_id=?,updated_at=? WHERE id=? AND organization_id=? AND is_active=1").bind(parentId, timestamp, userId, session.organization_id),
      env.DB.prepare("INSERT INTO audit_log(organization_id,user_id,action,correlation_id,details,created_at) SELECT ?,?,?,?,?,? WHERE changes()=1").bind(session.organization_id, session.sub, "user.parent.update", operationId, JSON.stringify({ target: userId, parent_user_id: parentId }), timestamp),
    ]);
    if (Number(batch[0]?.meta?.changes ?? 0) !== 1) return json({ error: "USER_NOT_FOUND", category: "DOMAIN", retryability: "NON_RETRYABLE" }, 404);
    if (Number(batch[1]?.meta?.changes ?? 0) !== 1) throw new Error("USER_PARENT_AUDIT_FAILED");
    return json({ success: true, parent_user_id: parentId });
  } catch {
    return json({ error: "USER_PARENT_UPDATE_UNAVAILABLE", category: "INFRASTRUCTURE", retryability: "TRANSIENT" }, 503);
  }
}

function canonicalHierarchy(rows: Record<string, unknown>[]) {
  return rows.map((row) => ({
    id: String(row.id),
    parent_user_id: typeof row.parent_user_id === "string" && row.parent_user_id.length > 0 ? row.parent_user_id : null,
    is_admin: row.is_admin === true || row.is_admin === 1 || row.is_admin === "1",
  }));
}
async function hierarchy(env:Env,session:Session){
  // The canonical All-Father principal uses a non-UUID sentinel id, while the
  // Rust/Android hierarchy parser requires UUID ids. It is not an org member
  // in the reporting tree, so represent its hierarchy as the valid empty array.
  if(session.role==="ALL_FATHER")return json([]);
  // Match the existing Rust API contract: every authenticated org member can
  // read the minimal active-org reporting tree. Direct reports must be present
  // for the client-side manager-authority cache to verify Task/Mission owners.
  const rows=await env.DB.prepare("SELECT id,parent_user_id,is_admin FROM users WHERE organization_id=? AND is_active=1 ORDER BY username")
    .bind(session.organization_id).all<Record<string,unknown>>();
  return json(canonicalHierarchy(rows.results??[]));
}
async function profiles(env:Env,session:Session,ownerId?:string){
  if(session.role==="STAFF"&&ownerId&&ownerId!==session.sub)return json({error:"FORBIDDEN",category:"AUTHORITY",retryability:"NON_RETRYABLE"},403);
  if(session.role==="STAFF"&&!ownerId)ownerId=session.sub;
  const rows=ownerId?await env.DB.prepare("SELECT id,state FROM aggregates WHERE organization_id=? AND aggregate_type=? AND id=? LIMIT 1").bind(session.organization_id,"staff_profile",ownerId).all<Record<string,unknown>>():await env.DB.prepare("SELECT id,state FROM aggregates WHERE organization_id=? AND aggregate_type=? ORDER BY updated_at DESC LIMIT 100").bind(session.organization_id,"staff_profile").all<Record<string,unknown>>();
  return json((rows.results??[]).map(r=>({id:r.id,...(safeJson(String(r.state)) as Json)})));
}


function validTimeWindow(body: Json): {start_at_ms:number;end_at_ms:number}|null {
  const start=body.start_at_ms, end=body.end_at_ms;
  if (!Number.isSafeInteger(start) || !Number.isSafeInteger(end) || Number(start)<0 || Number(end)<=Number(start)) return null;
  return {start_at_ms:Number(start),end_at_ms:Number(end)};
}
async function createTodoAggregate(request: Request, env: Env, session: Session, kind: "todo_list"|"target_list"|"staff_loan") {
  if(session.client_type==="mobile_observer")return json({error:"CLIENT_CAPABILITY_REQUIRED",category:"AUTHORITY",retryability:"NON_RETRYABLE"},403);
  const body=await request.json().catch(()=>null) as Json|null;
  if (!body || Array.isArray(body)) return json({error:"INVALID_TODO_REQUEST",category:"DOMAIN",retryability:"NON_RETRYABLE"},400);
  const id=uuid(), timestamp=Date.now(), operationId=uuid();
  const getUser=async (key:string) => {
    const value=body[key];
    if(typeof value!=="string" || !looksLikeUuid(value)) return null;
    return env.DB.prepare("SELECT id,organization_id,is_active,is_manager,role FROM users WHERE id=? AND organization_id=? AND is_active=1 LIMIT 1")
      .bind(value,session.organization_id).first<{id:string;organization_id:string;is_active:number;is_manager:number;role:string}>();
  };
  let state: Json, eventPayload: Json;
  if (kind==="todo_list") {
    if(Object.keys(body).some(key=>!["owner","origin","items"].includes(key)) || (body.origin!=="manager_assigned" && body.origin!=="staff_authored") ||
       !Array.isArray(body.items) || body.items.length>200 || !body.items.every(item=>!!item&&typeof item==="object"&&!Array.isArray(item)&&typeof (item as Json).description==="string"&&(item as Json).description.trim().length>0&&(item as Json).description.length<=1000)) {
      return json({error:"INVALID_TODO_LIST",category:"DOMAIN",retryability:"NON_RETRYABLE"},400);
    }
    const owner=await getUser("owner");
    if(!owner)return json({error:"INVALID_RESOURCE_OWNER",category:"AUTHORITY",retryability:"NON_RETRYABLE"},403);
    if(session.role==="STAFF" && (owner.id!==session.sub || body.origin!=="staff_authored"))return json({error:"RESOURCE_NOT_OWNED",category:"AUTHORITY",retryability:"NON_RETRYABLE"},403);
    if(session.role==="STAFF" && body.origin==="manager_assigned")return json({error:"FORBIDDEN",category:"AUTHORITY",retryability:"NON_RETRYABLE"},403);
    const items=(body.items as Json[]).map(item=>({item_id:uuid(),description:String(item.description).trim()}));
    eventPayload={owner:owner.id,origin:body.origin,items};
    state={id,version:1,lifecycle_epoch:0,authority_epoch:0,status:"Draft",owner:owner.id,owner_id:owner.id,origin:body.origin==="manager_assigned"?"ManagerAssigned":"StaffAuthored",items,created_by:session.sub,team_leader_pre_check:null,escalated_to:null};
  } else if(kind==="target_list") {
    if(Object.keys(body).some(key=>!["owner","origin","description","time_window"].includes(key)) ||
       (body.origin!=="manager_assigned"&&body.origin!=="staff_authored") || typeof body.description!=="string" || !body.description.trim() || body.description.length>2000 ||
       !body.time_window || typeof body.time_window!=="object" || Array.isArray(body.time_window)) return json({error:"INVALID_TARGET_LIST",category:"DOMAIN",retryability:"NON_RETRYABLE"},400);
    const owner=await getUser("owner"), window=validTimeWindow(body.time_window as Json);
    if(!owner)return json({error:"INVALID_RESOURCE_OWNER",category:"AUTHORITY",retryability:"NON_RETRYABLE"},403);
    if(!window)return json({error:"INVALID_TIME_WINDOW",category:"DOMAIN",retryability:"NON_RETRYABLE"},400);
    if(session.role==="STAFF"&&(owner.id!==session.sub||body.origin!=="staff_authored"))return json({error:"RESOURCE_NOT_OWNED",category:"AUTHORITY",retryability:"NON_RETRYABLE"},403);
    eventPayload={owner:owner.id,origin:body.origin,description:body.description.trim(),time_window:window};
    state={id,version:1,lifecycle_epoch:0,authority_epoch:0,status:"Draft",owner:owner.id,owner_id:owner.id,origin:body.origin==="manager_assigned"?"ManagerAssigned":"StaffAuthored",description:body.description.trim(),time_window:window,created_by:session.sub,team_leader_pre_check:null,escalated_to:null};
  } else {
    const allowed=["staff_user_id","real_owner_id","borrowing_manager_id","start_at_ms","end_at_ms"];
    if(Object.keys(body).some(key=>!allowed.includes(key)))return json({error:"INVALID_STAFF_LOAN",category:"DOMAIN",retryability:"NON_RETRYABLE"},400);
    const staff=await getUser("staff_user_id"), owner=await getUser("real_owner_id"), manager=await getUser("borrowing_manager_id");
    const start=body.start_at_ms,end=body.end_at_ms;
    if(!staff||!owner||!manager)return json({error:"INVALID_LOAN_PARTICIPANT",category:"AUTHORITY",retryability:"NON_RETRYABLE"},403);
    if(!Number.isSafeInteger(start)||!Number.isSafeInteger(end)||Number(start)<0||Number(end)<=Number(start))return json({error:"INVALID_TIME_WINDOW",category:"DOMAIN",retryability:"NON_RETRYABLE"},400);
    if(session.role==="STAFF"&&session.sub!==manager.id&&!(session.sub===staff.id&&session.sub===owner.id))return json({error:"FORBIDDEN",category:"AUTHORITY",retryability:"NON_RETRYABLE"},403);
    if(session.role==="STAFF"&&!(manager.is_manager===1))return json({error:"MANAGER_REQUIRED",category:"AUTHORITY",retryability:"NON_RETRYABLE"},403);
    eventPayload={staff_user_id:staff.id,real_owner_id:owner.id,borrowing_manager_id:manager.id,start_at_ms:Number(start),end_at_ms:Number(end)};
    state={id,version:1,lifecycle_epoch:0,authority_epoch:0,status:"Requested",staff_user_id:staff.id,real_owner_id:owner.id,borrowing_manager_id:manager.id,owner_id:staff.id,start_at_ms:Number(start),end_at_ms:Number(end),created_by:session.sub};
  }
  try {
    const event={event_id:uuid(),aggregate_id:id,aggregate_type:kind,event_type:kind==="todo_list"?"CreateTodoList":kind==="target_list"?"CreateTargetList":"RequestStaffLoan",aggregate_version:1,organization_id:session.organization_id,payload:eventPayload,occurred_at:timestamp,operation_id:operationId,correlation_id:operationId,causation_id:null,actor:{user_id:session.sub,organization_id:session.organization_id}};
    const result=await env.DB.batch([
      env.DB.prepare("INSERT INTO aggregates(id,aggregate_type,version,lifecycle_epoch,authority_epoch,state,updated_at,organization_id) VALUES(?,?,1,0,0,?,?,?)").bind(id,kind,JSON.stringify(state),timestamp,session.organization_id),
      env.DB.prepare("INSERT INTO domain_events(event_id,aggregate_id,aggregate_version,event_type,payload,occurred_at,operation_id,correlation_id,causation_id,actor,organization_id) VALUES(?,?,?,?,?,?,?,?,?,?,?)").bind(event.event_id,id,1,event.event_type,JSON.stringify(eventPayload),timestamp,operationId,operationId,null,JSON.stringify(event.actor),session.organization_id),
      env.DB.prepare("INSERT INTO audit_log(organization_id,user_id,action,correlation_id,details,created_at) VALUES(?,?,?,?,?,?)").bind(session.organization_id,session.sub,kind+".create",operationId,JSON.stringify({target:id}),timestamp),
    ]);
    if(Number(result[0]?.meta?.changes??0)!==1)throw new Error("TODO_AGGREGATE_CREATE_FAILED");
    const responseField=kind==="todo_list"?"todo_list_id":kind==="target_list"?"target_list_id":"staff_loan_id";
    return json({[responseField]:id},201);
  } catch {
    return json({error:"TODO_STORAGE_UNAVAILABLE",category:"INFRASTRUCTURE",retryability:"TRANSIENT"},503);
  }
}

type ProfileRecord = {
  profile_id: string; owner_id: string; full_name: string; photo_blob_key: string | null;
  job_title: string | null; contact_email: string | null; contact_phone: string | null;
  department: string | null; class_label: string | null; parent_display_name: string | null;
  team_memberships: string[]; work_stats?: { assigned_items_count: number | null; completed_items_count: number | null; target_summary: string | null };
};
function profileDto(state: Json, session: Session): Json {
  const dto: Json = {};
  for (const key of ["profile_id","owner_id","full_name","photo_blob_key","job_title","contact_email","contact_phone","department","class_label","parent_display_name","team_memberships"]) {
    if (Object.prototype.hasOwnProperty.call(state,key)) dto[key] = state[key];
  }
  if (session.role === "ORGANIZATION_ADMIN" || session.class === "top_level_manager") {
    dto.work_stats = state.work_stats && typeof state.work_stats === "object"
      ? state.work_stats
      : { assigned_items_count: null, completed_items_count: null, target_summary: null };
  }
  return dto;
}
async function allProfileRows(env: Env, session: Session) {
  const result = await env.DB.prepare("SELECT id,state,version FROM aggregates WHERE organization_id=? AND aggregate_type='staff_profile' ORDER BY updated_at DESC LIMIT 500")
    .bind(session.organization_id).all<{id:string;state:string;version:number}>();
  return (result.results ?? []).map(row => ({ ...row, state: safeJson(row.state) as Json }));
}
function validProfileBody(body: Json): boolean {
  const strings = ["owner_id","full_name"];
  if (strings.some(key => typeof body[key] !== "string" || !(body[key] as string).trim())) return false;
  for (const key of ["photo_blob_key","job_title","contact_email","contact_phone","department","class_label","parent_display_name"]) {
    if (body[key] !== undefined && body[key] !== null && typeof body[key] !== "string") return false;
  }
  return body.full_name !== undefined && (body.full_name as string).trim().length <= 200 &&
    (body.owner_id as string).length <= 128 &&
    (body.team_memberships === undefined || (Array.isArray(body.team_memberships) && body.team_memberships.length <= 100 && body.team_memberships.every(x => typeof x === "string" && x.length <= 100)));
}
async function upsertProfile(request: Request, env: Env, session: Session) {
  requireRole(session, "ORGANIZATION_ADMIN");
  const body = await request.json().catch(() => null) as Json | null;
  if (!body || Array.isArray(body) || !validProfileBody(body)) return json({ error: "PROFILE_DOMAIN_ERROR", message: "A valid owner_id and full_name are required.", category: "DOMAIN", retryability: "NON_RETRYABLE" }, 400);
  const ownerId = String(body.owner_id).trim();
  const owner = await env.DB.prepare("SELECT id,class,parent_user_id FROM users WHERE id=? AND organization_id=? AND is_active=1 AND id<>'__allfather__' LIMIT 1")
    .bind(ownerId, session.organization_id).first<{id:string;class:string|null;parent_user_id:string|null}>();
  if (!owner) return json({ error: "PROFILE_OWNER_NOT_FOUND", category: "DOMAIN", retryability: "NON_RETRYABLE" }, 404);
  const profiles = await allProfileRows(env, session);
  const existing = profiles.find(row => row.state && String(row.state.owner_id ?? "") === ownerId);
  const profileId = existing ? String(existing.state.profile_id ?? existing.id) : uuid();
  const profile: ProfileRecord = {
    profile_id: profileId, owner_id: ownerId, full_name: String(body.full_name).trim(),
    photo_blob_key: typeof body.photo_blob_key === "string" ? body.photo_blob_key.trim() || null : null,
    job_title: typeof body.job_title === "string" ? body.job_title.trim() || null : null,
    contact_email: typeof body.contact_email === "string" ? body.contact_email.trim() || null : null,
    contact_phone: typeof body.contact_phone === "string" ? body.contact_phone.trim() || null : null,
    department: typeof body.department === "string" ? body.department.trim() || null : null,
    class_label: typeof body.class_label === "string" ? body.class_label.trim() || null : owner.class,
    parent_display_name: typeof body.parent_display_name === "string" ? body.parent_display_name.trim() || null : null,
    team_memberships: Array.isArray(body.team_memberships) ? body.team_memberships.map(x => String(x).trim()).filter(Boolean) : [],
    work_stats: existing?.state?.work_stats && typeof existing.state.work_stats === "object"
      ? existing.state.work_stats as ProfileRecord["work_stats"]
      : { assigned_items_count: null, completed_items_count: null, target_summary: null },
  };
  const timestamp = Date.now(), operationId = uuid(), version = existing ? Number(existing.version || 1) + 1 : 1;
  try {
    const write = existing
      ? env.DB.prepare("UPDATE aggregates SET state=?,version=?,updated_at=? WHERE id=? AND organization_id=? AND aggregate_type='staff_profile'")
          .bind(JSON.stringify(profile), version, timestamp, existing.id, session.organization_id)
      : env.DB.prepare("INSERT INTO aggregates(id,aggregate_type,version,lifecycle_epoch,authority_epoch,state,updated_at,organization_id) VALUES(?,'staff_profile',1,0,0,?,?,?)")
          .bind(profileId, JSON.stringify(profile), timestamp, session.organization_id);
    const eventId=uuid(), aggregateId=existing?existing.id:profileId;
    const eventPayload=JSON.stringify(profile);
    const batch = await env.DB.batch([
      write,
      env.DB.prepare("INSERT INTO domain_events(event_id,aggregate_id,aggregate_version,event_type,payload,occurred_at,vector_clock,operation_id,correlation_id,causation_id,actor,organization_id) SELECT ?,?,?,?,?,?,'{}',?,?,NULL,?,? WHERE EXISTS (SELECT 1 FROM aggregates WHERE id=? AND organization_id=? AND aggregate_type='staff_profile' AND version=?)")
        .bind(eventId,aggregateId,version,existing?"ProfileUpdated":"ProfileCreated",eventPayload,timestamp,operationId,operationId,JSON.stringify({user_id:session.sub,organization_id:session.organization_id}),session.organization_id,aggregateId,session.organization_id,version),
      env.DB.prepare("INSERT INTO audit_log(organization_id,user_id,action,correlation_id,details,created_at) SELECT ?,?,?,?,?,? WHERE changes()=1")
        .bind(session.organization_id, session.sub, existing ? "profile.update" : "profile.create", operationId, JSON.stringify({ owner_id: ownerId, profile_id: profileId }), timestamp),
    ]);
    if (Number(batch[0]?.meta?.changes ?? 0) !== 1 || Number(batch[1]?.meta?.changes ?? 0) !== 1 || Number(batch[2]?.meta?.changes ?? 0) !== 1) throw new Error("PROFILE_WRITE_FAILED");
    return json(profileDto(profile as unknown as Json, session), existing ? 200 : 201);
  } catch {
    return json({ error: "PROFILE_STORE_UNAVAILABLE", category: "INFRASTRUCTURE", retryability: "TRANSIENT" }, 503);
  }
}
async function listProfiles(env: Env, session: Session, ownerId?: string) {
  const rows = await allProfileRows(env, session);
  const profiles = rows.filter(row => row.state && (!ownerId || String(row.state.owner_id ?? "") === ownerId));
  if (ownerId && profiles.length === 0) return json({ error: "PROFILE_NOT_FOUND", category: "DOMAIN", retryability: "NON_RETRYABLE" }, 404);
  return json(ownerId ? profileDto(profiles[0].state, session) : profiles.map(row => profileDto(row.state, session)));
}
function parseCsvRows(input: string): Json[] {
  const rows: string[][] = [];
  let row: string[] = [], field = "", quoted = false;
  for (let i=0; i<input.length; i++) {
    const c=input[i];
    if (quoted && c === '"' && input[i+1] === '"') { field += '"'; i++; }
    else if (c === '"') quoted = !quoted;
    else if (c === ',' && !quoted) { row.push(field); field=""; }
    else if ((c === '\n' || c === '\r') && !quoted) {
      if (c === '\r' && input[i+1] === '\n') i++;
      row.push(field); if (row.some(value => value.length)) rows.push(row); row=[]; field="";
    } else field += c;
  }
  row.push(field); if (row.some(value => value.length)) rows.push(row);
  if (rows.length < 2) return [];
  const headers=rows[0].map(value=>value.trim());
  return rows.slice(1).map(values => {
    const obj: Json = {};
    headers.forEach((key,index)=>{ if (key === "team_memberships") obj[key]=(values[index]||"").split(";").map(x=>x.trim()).filter(Boolean); else obj[key]=values[index]||null; });
    return obj;
  });
}
async function importProfiles(request: Request, env: Env, session: Session) {
  requireRole(session, "ORGANIZATION_ADMIN");
  const contentType=(request.headers.get("content-type")||"text/csv").toLowerCase();
  const raw=await request.text();
  if (raw.length > 2_000_000) return json({ error:"IMPORT_TOO_LARGE",category:"DOMAIN",retryability:"NON_RETRYABLE" },413);
  let rows: Json[];
  try {
    if (contentType.includes("json")) { const parsed=JSON.parse(raw); rows=Array.isArray(parsed)?parsed:[]; }
    else rows=parseCsvRows(raw);
  } catch { return json({ error:"INVALID_PROFILE_IMPORT",category:"DOMAIN",retryability:"NON_RETRYABLE" },400); }
  if (rows.length > 500) return json({ error:"IMPORT_TOO_MANY_ROWS",category:"DOMAIN",retryability:"NON_RETRYABLE" },400);
  let created=0, updated=0, failed=0;
  const results:Json[]=[];
  for (let index=0; index<rows.length; index++) {
    const row=rows[index];
    const userId=row&&typeof row.user_id==="string"?row.user_id:null;
    if (!row || Array.isArray(row) || typeof row!=="object") {
      failed++; results.push({outcome:"failed",user_id:userId,reason:"INVALID_PROFILE_ROW"}); continue;
    }
    const ownerId=typeof row.user_id==="string"?row.user_id:row.owner_id;
    const normalized:Json={...row,owner_id:ownerId};
    if (typeof ownerId!=="string" || !looksLikeUuid(ownerId) ||
        ["full_name","contact_email","contact_phone","job_title","department"].some(key=>typeof row[key]!=="string"||!(row[key] as string).trim()) ||
        !validProfileBody(normalized)) {
      failed++; results.push({outcome:"failed",user_id:userId,reason:"INVALID_PROFILE_ROW"}); continue;
    }
    const response=await upsertProfile(new Request(request.url,{method:"PUT",headers:{"content-type":"application/json"},body:JSON.stringify(normalized)}),env,session);
    if (response.ok) {
      const isCreated=response.status===201;
      if(isCreated)created++;else updated++;
      results.push({outcome:isCreated?"created":"updated",user_id:ownerId});
    } else {
      const error=await response.json().catch(()=>({error:"PROFILE_IMPORT_FAILED"})) as Json;
      failed++; results.push({outcome:"failed",user_id:ownerId,reason:String(error.error||"PROFILE_IMPORT_FAILED")});
    }
  }
  return json({total_rows:rows.length,created,updated,failed,results});
}

async function exportProfiles(request: Request, env: Env, session: Session) {
  requireRole(session, "ORGANIZATION_ADMIN");
  const rows=await allProfileRows(env,session);
  const profiles=rows.filter(row=>row.state).map(row=>({
    user_id:String(row.state.owner_id??""),
    full_name:String(row.state.full_name??""),
    contact_email:String(row.state.contact_email??""),
    contact_phone:String(row.state.contact_phone??""),
    job_title:String(row.state.job_title??""),
    department:String(row.state.department??""),
    photo_blob_key:row.state.photo_blob_key??null,
    class_label:row.state.class_label??null,
    parent_display_name:row.state.parent_display_name??null,
    team_memberships:Array.isArray(row.state.team_memberships)?row.state.team_memberships:[],
  } as Json));
  const format=new URL(request.url).searchParams.get("format")||"csv";
  if (format==="json") return json(profiles);
  if (format!=="csv") return json({error:"INVALID_EXPORT_FORMAT",category:"DOMAIN",retryability:"NON_RETRYABLE"},400);
  const fields=["user_id","full_name","contact_email","contact_phone","job_title","department","photo_blob_key","class_label","parent_display_name","team_memberships"];
  const escape=(value:unknown)=>{const text=Array.isArray(value)?value.join(";"):value==null?"":String(value);return /[",\r\n]/.test(text)?'"'+text.replace(/"/g,'""')+'"':text;};
  const csv=[fields.join(","),...profiles.map(profile=>fields.map(key=>escape(profile[key])).join(","))].join("\r\n");
  return new Response(csv,{status:200,headers:{"content-type":"text/csv; charset=utf-8","content-disposition":"attachment; filename=onyx-staff-profiles.csv","cache-control":"no-store"}});
}

async function pushSubscribe(request:Request,env:Env,session:Session){
  const body=await request.json().catch(()=>null) as Json|null;
  if(!body||typeof body.endpoint!=="string"||!body.endpoint||typeof body.p256dh!=="string"||!body.p256dh||typeof body.auth!=="string"||!body.auth)
    return json({error:"INVALID_PUSH_SUBSCRIPTION"},400);
  const id=typeof body.id==="string"&&body.id.trim()?body.id.trim():uuid();
  if(id.length>256||body.endpoint.length>4096||body.p256dh.length>1024||body.auth.length>1024)
    return json({error:"INVALID_PUSH_SUBSCRIPTION"},400);
  const result=await env.DB.prepare("INSERT INTO push_subscriptions(id,user_id,organization_id,endpoint,p256dh,auth,platform,created_at) VALUES(?,?,?,?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET endpoint=excluded.endpoint,p256dh=excluded.p256dh,auth=excluded.auth,platform=excluded.platform WHERE push_subscriptions.user_id=excluded.user_id AND push_subscriptions.organization_id=excluded.organization_id")
    .bind(id,session.sub,session.organization_id,body.endpoint,body.p256dh,body.auth,typeof body.platform==="string"?body.platform.slice(0,32):"web",Date.now()).run();
  if(Number(result.meta?.changes??0)!==1)return json({error:"SUBSCRIPTION_CONFLICT",category:"AUTHORITY",retryability:"NON_RETRYABLE"},409);
  return json({id,success:true});
}
async function pushDelete(env:Env,session:Session,id:string){await env.DB.prepare("DELETE FROM push_subscriptions WHERE id=? AND user_id=? AND organization_id=?").bind(id,session.sub,session.organization_id).run();return json({success:true});}
async function fileDownload(env:Env,session:Session,hash:string){
  // The current file_assets schema has tenant scope but no per-user owner or
  // ACL columns. Until ACL parity exists, Staff file downloads fail closed.
  requireRole(session,"ALL_FATHER","ORGANIZATION_ADMIN");
  const asset=await env.DB.prepare("SELECT blob_key,content_type FROM file_assets WHERE content_hash=? AND organization_id=? LIMIT 1").bind(hash,session.organization_id).first<{blob_key:string;content_type:string|null}>();if(!asset)return json({error:"FILE_NOT_FOUND"},404);const endpoint=env.HF_S3_ENDPOINT,bucket=env.HF_S3_BUCKET,access=env.HF_S3_ACCESS_KEY_ID,secret=env.HF_S3_SECRET_ACCESS_KEY;if(!endpoint||!bucket||!access||!secret)return json({error:"BLOB_STORE_NOT_CONFIGURED"},503);const response=await signedS3Get(endpoint,bucket,asset.blob_key,access,secret);if(!response.ok)return json({error:"BLOB_STORE_UNAVAILABLE"},502);const headers=new Headers();headers.set("cache-control","private, max-age=60");if(asset.content_type)headers.set("content-type",asset.content_type);return new Response(response.body,{status:200,headers});}
async function hmacBytes(keyBytes:Uint8Array,value:string):Promise<Uint8Array>{const key=await crypto.subtle.importKey("raw",keyBytes,{name:"HMAC",hash:"SHA-256"},false,["sign"]);return new Uint8Array(await crypto.subtle.sign("HMAC",key,new TextEncoder().encode(value)));}
async function signedS3Get(endpoint:string,bucket:string,key:string,access:string,secret:string){const base=endpoint.replace(/\/$/,"");const path=`/${bucket}/${key.split("/").map(encodeURIComponent).join("/")}`;const url=`${base}${path}`;const host=new URL(base).host;const bodyHash=await sha256("");const date=new Date();const amzDate=date.toISOString().replace(/[-:]/g,"").replace(/\.\d{3}Z$/,"");const short=amzDate.slice(0,8);const canonicalHeaders=`host:${host}\nx-amz-content-sha256:${bodyHash}\nx-amz-date:${amzDate}\n`;const signedHeaders="host;x-amz-content-sha256;x-amz-date";const canonicalRequest=`GET\n${path}\n\n${canonicalHeaders}\n${signedHeaders}\n${bodyHash}`;const scope=`${short}/us-east-1/s3/aws4_request`;const stringToSign=`AWS4-HMAC-SHA256\n${amzDate}\n${scope}\n${await sha256(canonicalRequest)}`;const kDate=await hmacBytes(new TextEncoder().encode(`AWS4${secret}`),short);const kRegion=await hmacBytes(kDate,"us-east-1");const kService=await hmacBytes(kRegion,"s3");const signingKey=await hmacBytes(kService,"aws4_request");const signature=Array.from(await hmacBytes(signingKey,stringToSign),(b)=>b.toString(16).padStart(2,"0")).join("");return fetch(url,{headers:{Host:host,"x-amz-content-sha256":bodyHash,"x-amz-date":amzDate,Authorization:`AWS4-HMAC-SHA256 Credential=${access}/${scope}, SignedHeaders=${signedHeaders}, Signature=${signature}`}});}

const WS_AUTH_PROTOCOL = "onyx-bearer";

function websocketBearer(request: Request): string | null {
  const header = request.headers.get("sec-websocket-protocol");
  if (!header || header.length > 20_000) return null;
  const protocols = header.split(",").map((value) => value.trim()).filter(Boolean);
  const authProtocols = protocols.filter((value) => value === WS_AUTH_PROTOCOL);
  const credentials = protocols.filter((value) => value !== WS_AUTH_PROTOCOL);
  if (authProtocols.length !== 1 || credentials.length !== 1) return null;
  const token = credentials[0];
  return token.length <= 16_384 && /^[A-Za-z0-9._~-]+$/.test(token) ? token : null;
}

async function websocketEvents(request: Request, env: Env): Promise<Response> {
  const origin = request.headers.get("origin");
  const allowedOrigins = env.ONYX_CORS_ORIGINS?.split(",").map((value) => value.trim()).filter(Boolean) ?? [];
  if (origin && (!allowedOrigins.includes(origin) || allowedOrigins.includes("*"))) {
    return json({ error: "ORIGIN_NOT_ALLOWED", category: "AUTHORITY", retryability: "NON_RETRYABLE" }, 403);
  }
  if (request.headers.get("upgrade")?.toLowerCase() !== "websocket" ||
      !request.headers.get("connection")?.toLowerCase().split(",").map((value) => value.trim()).includes("upgrade")) {
    return json({ error: "WEBSOCKET_UPGRADE_REQUIRED", category: "DOMAIN", retryability: "NON_RETRYABLE" }, 426);
  }
  const token = websocketBearer(request);
  if (!token) return json({ error: "UNAUTHORIZED", category: "AUTHORITY", retryability: "NON_RETRYABLE" }, 401);
  try {
    const headers = new Headers(request.headers);
    headers.set("authorization", "Bearer " + token);
    const sessionRequest = new Request(request, { headers });
    const session = await requireSession(sessionRequest, env);
    // Preserve the Worker's existing event-stream authority boundary until
    // event payloads have resource-level Staff filtering.
    requireRole(session,"ALL_FATHER","ORGANIZATION_ADMIN");
    headers.set("x-onyx-authenticated-org", session.organization_id);
    headers.set("x-onyx-authenticated-user", session.sub);
    const hubId = env.EVENTS_HUB.idFromName(session.organization_id);
    return await env.EVENTS_HUB.get(hubId).fetch(new Request(request, { headers }));
  } catch (error) {
    const message = error instanceof Error ? error.message : "";
    if (message === "FORBIDDEN_ROLE") return json({ error: "FORBIDDEN", category: "AUTHORITY", retryability: "NON_RETRYABLE" }, 403);
    if (message === "AUTH_REQUIRED" || message.includes("token") || message.includes("expired") || message.includes("revoked")) {
      return json({ error: "UNAUTHORIZED", category: "AUTHORITY", retryability: "NON_RETRYABLE" }, 401);
    }
    console.error("WebSocket authentication unavailable");
    return json({ error: "AUTHENTICATION_SERVICE_UNAVAILABLE", category: "INFRASTRUCTURE", retryability: "TRANSIENT" }, 503);
  }
}

async function dispatch(request:Request,env:Env):Promise<Response>{
  const url=new URL(request.url);const origin=corsOrigin(request,env);
  if(request.method==="OPTIONS")return new Response(null,{status:204,headers:{"access-control-allow-origin":origin??"*","access-control-allow-methods":"GET,POST,PUT,DELETE,OPTIONS","access-control-allow-headers":"Authorization,Content-Type,Idempotency-Key","access-control-allow-credentials":"true","access-control-max-age":"86400"}});
  if(url.pathname==="/health")return json({status:"ok",service:"onyx-cloudflare-worker",runtime:"cloudflare-workers-free"},200,origin);
  if(url.pathname==="/ready"){
    try{
      const missing=await missingAuthSchema(env.DB);
      if(missing.length>0)return json({status:"degraded",database:"d1",schema:"incomplete",missing},503,origin);
      return json({status:"ok",service:"onyx-cloudflare-worker",database:"d1",schema:"ok"},200,origin);
    }catch{
      return json({status:"degraded",database:"unavailable",schema:"unknown"},503,origin);
    }
  }
  try{
    if(url.pathname==="/api/events"&&request.method==="GET")return await websocketEvents(request,env);
    if(url.pathname.startsWith("/api/relay/")&&request.method==="GET")return await relaySocket(request,env);
    if(url.pathname==="/api/auth/login"&&request.method==="POST")return await authLogin(request,env);
    if((url.pathname==="/api/auth/supabase"||url.pathname==="/api/auth/clerk")&&request.method==="POST")return await authSupabaseAllFather(request,env);
    if(url.pathname==="/api/auth/refresh"&&request.method==="POST")return await authRefresh(request,env);
    if(url.pathname==="/api/auth/logout"&&request.method==="POST")return await authLogout(request,env);
    const session=await requireSession(request,env);
    if(url.pathname==="/api/query"&&request.method==="GET")return await queryRoute(request,env,session);
    if(url.pathname==="/api/command"&&request.method==="POST")return await commandRoute(request,env,session);
    if(url.pathname==="/api/relay-ticket"&&request.method==="POST")return await issueRelayTicket(request,env,session);
    if(url.pathname==="/api/users"&&request.method==="GET")return await listUsers(env,session);
    if(url.pathname==="/api/users/hierarchy"&&request.method==="GET")return await hierarchy(env,session);
    if(url.pathname==="/api/admin/users"&&request.method==="GET")return await listAdminUsers(env,session);
    if(/^\/api\/admin\/users\/[^/]+\/deactivate$/.test(url.pathname)&&request.method==="POST")return await setUserActive(env,session,url.pathname.split("/").at(-2)!,false);
    if(/^\/api\/admin\/users\/[^/]+\/activate$/.test(url.pathname)&&request.method==="POST")return await setUserActive(env,session,url.pathname.split("/").at(-2)!,true);
    if(/^\/api\/admin\/users\/[^/]+\/password$/.test(url.pathname)&&request.method==="POST")return await setUserPassword(request,env,session,url.pathname.split("/").at(-2)!);
    if(/^\/api\/admin\/users\/[^/]+\/manager$/.test(url.pathname)&&request.method==="POST")return await setUserManager(request,env,session,url.pathname.split("/").at(-2)!);
    if(/^\/api\/admin\/users\/[^/]+\/parent$/.test(url.pathname)&&request.method==="POST")return await setUserParent(request,env,session,url.pathname.split("/").at(-2)!);
    if(url.pathname==="/api/admin/mobile-access"&&request.method==="GET")return await getMobileAccess(env,session);
    if(url.pathname==="/api/admin/mobile-access"&&request.method==="PUT")return await putMobileAccess(request,env,session);
    if(url.pathname==="/api/admin/policies"&&request.method==="POST")return await postAdminPolicy(request,env,session);
    if(url.pathname==="/api/admin/legal-holds"&&request.method==="POST")return await postAdminLegalHold(request,env,session);
    if(url.pathname==="/api/todo/lists"&&request.method==="POST")return await createTodoAggregate(request,env,session,"todo_list");
    if(url.pathname==="/api/todo/targets"&&request.method==="POST")return await createTodoAggregate(request,env,session,"target_list");
    if(url.pathname==="/api/todo/staff-loans"&&request.method==="POST")return await createTodoAggregate(request,env,session,"staff_loan");
    if(/^\/api\/admin\/users\/[^/]+\/class$/.test(url.pathname)&&request.method==="POST")return await assignUserClass(request,env,session,url.pathname.split("/").at(-2)!);
    if(url.pathname==="/api/profiles"&&request.method==="GET")return await listProfiles(env,session);
    if(url.pathname.startsWith("/api/profiles/")&&request.method==="GET")return await listProfiles(env,session,decodeURIComponent(url.pathname.split("/").pop()!));
    if(url.pathname==="/api/admin/profiles"&&request.method==="PUT")return await upsertProfile(request,env,session);
    if(url.pathname==="/api/admin/profiles/import"&&request.method==="POST")return await importProfiles(request,env,session);
    if(url.pathname==="/api/admin/profiles/export"&&request.method==="GET")return await exportProfiles(request,env,session);
    if(url.pathname==="/api/push/subscriptions"&&request.method==="POST")return await pushSubscribe(request,env,session);
    if(url.pathname.startsWith("/api/push/subscriptions/")&&request.method==="DELETE")return await pushDelete(env,session,url.pathname.split("/").pop()!);
    if(url.pathname.startsWith("/api/files/")&&request.method==="GET")return await fileDownload(env,session,url.pathname.split("/").pop()!);

    if(url.pathname==="/api/allfather/organizations"&&request.method==="POST")return await createOrganization(request,env,session);
    if(url.pathname==="/api/allfather/admins"&&request.method==="POST")return await createUser(request,env,session,"ORGANIZATION_ADMIN");
    if(url.pathname==="/api/admin/staff"&&request.method==="POST")return await createUser(request,env,session,"STAFF");
    if(url.pathname.startsWith("/api/admin/users/")&&request.method==="DELETE")return await deactivateUser(request,env,session,url.pathname.split("/").pop()!);
    return json({error:"ROUTE_NOT_MIGRATED",route:url.pathname,method:request.method,message:"This feature is not available on the Cloudflare Worker yet. The equivalent Axum route exists but has not been ported with equivalent authorization and persistence semantics.",category:"FEATURE_UNAVAILABLE",retryability:"NON_RETRYABLE",correlation_id:uuid()},501,origin);
  }catch(error){const message=error instanceof Error?error.message:"internal error";if(message==="AUTH_REQUIRED"||message.includes("token")||message.includes("expired")||message.includes("revoked"))return json({error:"UNAUTHORIZED",category:"AUTHORITY",retryability:"NON_RETRYABLE"},401,origin);if(message==="FORBIDDEN_ROLE")return json({error:"FORBIDDEN",category:"AUTHORITY",retryability:"NON_RETRYABLE"},403,origin);if(message.startsWith("INVALID_")||message==="PASSWORD_TOO_SHORT"||message==="ORGANIZATION_NOT_FOUND")return json({error:message,category:"DOMAIN",retryability:"NON_RETRYABLE"},400,origin);console.error(error);return json({error:"INTERNAL_SERVER_ERROR",category:"INFRASTRUCTURE",retryability:"TRANSIENT"},500,origin);}
}
/**
 * Apply one exact-origin CORS policy to every route, including helper
 * responses that may have been created without the caller's origin. Never
 * return wildcard CORS headers for credential-capable API responses.
 */
function applyCors(request:Request,env:Env,response:Response):Response{
  // A WebSocket upgrade carries a platform-managed webSocket property that
  // cannot be reconstructed by the standard Response constructor. Origin was
  // validated before forwarding the upgrade to the internal Durable Object.
  if (response.status === 101) return response;
  const headers=new Headers(response.headers);
  headers.delete("access-control-allow-origin");
  headers.delete("access-control-allow-credentials");
  const origin=request.headers.get("origin");
  const allowed=env.ONYX_CORS_ORIGINS?.split(",").map(x=>x.trim()).filter(Boolean)??[];
  if(origin&&allowed.includes(origin)&&!allowed.includes("*")){
    headers.set("access-control-allow-origin",origin);
    headers.set("access-control-allow-credentials","true");
    const vary=headers.get("vary");
    if(!vary)headers.set("vary","Origin");
    else if(!vary.split(",").some(value=>value.trim().toLowerCase()==="origin"))headers.set("vary",`${vary}, Origin`);
  }
  return new Response(response.body,{status:response.status,statusText:response.statusText,headers});
}
export default {fetch:async(request:Request,env:Env)=>applyCors(request,env,await dispatch(request,env))};
