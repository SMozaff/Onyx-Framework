from pathlib import Path
import re

ROOT = Path("deploy/cloudflare/edge-gateway/src/index.ts")
text = ROOT.read_text()

text = text.replace('  CLERK_ISSUER?: string;\n  CLERK_JWKS_URL?: string;\n  CLERK_SECRET_KEY?: string;', '  SUPABASE_URL?: string;\n  SUPABASE_JWKS_URL?: string;\n  SUPABASE_PUBLISHABLE_KEY?: string;\n  SUPABASE_SECRET_KEY?: string;')
text = text.replace('import { argon2id, argon2Verify } from "hash-wasm";\n', 'import { argon2id, argon2Verify } from "hash-wasm";\nimport { adminCreateUser, adminSetUserBanned, passwordSignIn, verifySupabaseJwt } from "./supabase";\n')


def replace_block(pattern: str, replacement: str):
    global text
    new_text, count = re.subn(pattern, replacement, text, count=1)
    if count != 1:
        raise SystemExit(f"Could not replace block: {pattern}")
    text = new_text


auth_login = r'''async function authLogin(request:Request,env:Env){
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
'''
replace_block(r'async function authLogin\(request:Request,env:Env\)\{[\s\S]*?(?=async function authRefresh)', auth_login)

supabase_allfather = r'''async function authSupabaseAllFather(request:Request,env:Env){
  const token=authorization(request);
  if(!token)return json({error:"AUTH_REQUIRED",category:"AUTHORITY",retryability:"NON_RETRYABLE"},401);
  try{
    const claims=await verifySupabaseJwt(env,token);
    const email=typeof claims.email==="string"?claims.email.trim().toLowerCase():"";
    const verified=claims.email_verified;
    const expected=(env.ONYX_ALLFATHER_EMAIL||ALLFATHER_EMAIL).trim().toLowerCase();
    if(!email||verified===false||email!==expected)return json({error:"ALLFATHER_REQUIRED",category:"AUTHORITY",retryability:"NON_RETRYABLE"},403);
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
'''
replace_block(r'function clerkIssuer\(env:Env\)\{[\s\S]*?(?=function requireRole)', supabase_allfather)

create_user = r'''async function createUser(request:Request,env:Env,session:Session,targetRole:"ORGANIZATION_ADMIN"|"STAFF"){
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
'''
replace_block(r'async function createUser\(request:Request,env:Env,session:Session,targetRole:"ORGANIZATION_ADMIN"\|"STAFF"\)\{[\s\S]*?(?=async function deactivateUser)', create_user)

deactivate_user = r'''async function deactivateUser(request:Request,env:Env,session:Session,userId:string){
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
'''
replace_block(r'async function deactivateUser\(request:Request,env:Env,session:Session,userId:string\)\{[\s\S]*?(?=async function )', deactivate_user)

text = text.replace('authClerkAllFather', 'authSupabaseAllFather')
text = text.replace('if(url.pathname==="/api/auth/clerk"&&request.method==="POST")return authSupabaseAllFather(request,env);', 'if((url.pathname==="/api/auth/supabase"||url.pathname==="/api/auth/clerk")&&request.method==="POST")return authSupabaseAllFather(request,env);')
ROOT.write_text(text)
print("migrated", ROOT)
