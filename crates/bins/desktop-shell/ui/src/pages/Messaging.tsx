import { useState } from "react";
import { useParams, useNavigate } from "react-router-dom";
import type { Id16, LoadedAggregate } from "@/types/onyx";
import { newId16 } from "@/types/onyx";
import { useQuery } from "@/hooks/useQuery";
import { useCommand } from "@/hooks/useCommand";
import { useSession } from "@/hooks/useSession";

/**
 * Conversation list-by-id + thread view, plus a Connections panel for
 * sending/accepting/declining contact requests. Same "no list/projection
 * query exists yet" caveat as `Missions.tsx`/`Tasks.tsx` applies — this
 * page is id-driven (paste/select a known conversation id, or create
 * one), not a live inbox. Covers the full `communication-domain` command
 * surface (`Conversation` + `Message` + `ConnectionRequest`), including
 * the Phase 1 additions: Supergroup/SubTeam conversation types and the
 * "supergroup membership required first" invariant.
 *
 * Visual tone is deliberately plain and utilitarian, matching the rest
 * of this shell's `onyx-*` design tokens (dark, muted, no illustration
 * or playful chrome) — an explicit product decision that this is a
 * professional work tool, not a consumer chat app.
 */
export default function Messaging() {
  const { conversationId } = useParams<{ conversationId?: string }>();
  const navigate = useNavigate();
  const session = useSession();

  const targetId: Id16 | null = conversationId ? JSON.parse(conversationId) : null;
  const {
    data: conversation,
    loading: loadingConversation,
    error: conversationError,
    refetch: refetchConversation,
  } = useQuery<LoadedAggregate>("GetConversation", targetId);

  return (
    <div className="max-w-4xl">
      <h1 className="text-xl font-semibold text-onyx-text">Messaging</h1>

      <div className="mt-4 grid grid-cols-[minmax(0,2fr)_minmax(0,1fr)] gap-6">
        <div>
          <IdLookup onLookup={(id) => navigate(`/messaging/${JSON.stringify(id)}`)} />
          <CreateConversationForm
            session={session}
            onCreated={(id) => navigate(`/messaging/${JSON.stringify(id)}`)}
          />

          {targetId && (
            <div className="mt-6 rounded-lg border border-onyx-border bg-onyx-surface p-4">
              {loadingConversation && <p className="text-sm text-onyx-text-dim">Loading…</p>}
              {conversationError && (
                <p className="text-sm text-onyx-status-blocked">{conversationError.message}</p>
              )}
              {!loadingConversation && !conversationError && conversation === null && (
                <p className="text-sm text-onyx-text-dim">No conversation found for this id.</p>
              )}
              {conversation && (
                <ConversationPanel
                  targetId={targetId}
                  conversation={conversation}
                  session={session}
                  onChanged={() => void refetchConversation()}
                />
              )}
            </div>
          )}
        </div>

        <ConnectionsPanel session={session} />
      </div>
    </div>
  );
}

function IdLookup({ onLookup }: { onLookup: (id: Id16) => void }) {
  const [raw, setRaw] = useState("");
  const [parseError, setParseError] = useState<string | null>(null);

  function submit() {
    try {
      const parsed = JSON.parse(raw);
      if (!Array.isArray(parsed) || parsed.length !== 16) {
        throw new Error("expected a JSON array of 16 numbers (the wire shape of an ObjectId)");
      }
      setParseError(null);
      onLookup(parsed as Id16);
    } catch (e) {
      setParseError(e instanceof Error ? e.message : String(e));
    }
  }

  return (
    <div>
      <label className="block text-xs font-medium text-onyx-text-dim">
        Look up by conversation id (JSON array of 16 bytes)
      </label>
      <div className="mt-1 flex gap-2">
        <input
          value={raw}
          onChange={(e) => setRaw(e.target.value)}
          placeholder="[12,34,...]"
          className="flex-1 rounded-md border border-onyx-border bg-onyx-surface px-3 py-1.5 text-sm text-onyx-text focus:border-onyx-accent focus:outline-none"
        />
        <button
          type="button"
          onClick={submit}
          className="rounded-md bg-onyx-surface px-3 py-1.5 text-sm text-onyx-text hover:bg-onyx-surface-hover"
        >
          Open
        </button>
      </div>
      {parseError && <p className="mt-1 text-xs text-onyx-status-blocked">{parseError}</p>}
    </div>
  );
}

/** The four `ConversationType` variants `communication_domain::value::ConversationType`
 * defines — see that enum's own doc comment for what each means, in
 * particular `Supergroup`/`SubTeam`'s Phase 1 semantics. */
const CONVERSATION_TYPES = ["Direct", "Channel", "Supergroup", "SubTeam"] as const;
type ConversationTypeOption = (typeof CONVERSATION_TYPES)[number];

function CreateConversationForm({
  session,
  onCreated,
}: {
  session: ReturnType<typeof useSession>;
  onCreated: (id: Id16) => void;
}) {
  const [conversationType, setConversationType] = useState<ConversationTypeOption>("Channel");
  const [parentSupergroupRaw, setParentSupergroupRaw] = useState("");
  const { execute, loading, error } = useCommand();
  const [formError, setFormError] = useState<string | null>(null);

  async function create() {
    setFormError(null);

    // `parent_supergroup` is required iff conversationType is "SubTeam"
    // — `Conversation::create()` rejects any other combination (see
    // `communication_domain::aggregate::Conversation::create`'s doc
    // comment). Validated here too so the error is immediate rather
    // than a round trip to the backend for an obviously-wrong input.
    let parentSupergroup: Id16 | null = null;
    if (conversationType === "SubTeam") {
      try {
        const parsed = JSON.parse(parentSupergroupRaw);
        if (!Array.isArray(parsed) || parsed.length !== 16) {
          throw new Error("expected a JSON array of 16 numbers");
        }
        parentSupergroup = parsed as Id16;
      } catch (e) {
        setFormError(
          `A SubTeam requires its parent Supergroup's id: ${
            e instanceof Error ? e.message : String(e)
          }`,
        );
        return;
      }
    }

    const result = (await execute({
      commandType: "CreateConversation",
      targetId: newId16(), // ignored by CreationHandler but must be a valid 16-byte ObjectId.
      targetType: "conversation",
      organizationId: session.organizationId,
      userId: session.userId,
      deviceId: session.deviceId,
      expectedVersion: 0,
      payload: {
        CreateConversation: {
          conversation_type: conversationType,
          parent_supergroup: parentSupergroup,
        },
      },
    })) as { conversation_id?: Id16 };

    if (result.conversation_id) {
      setParentSupergroupRaw("");
      onCreated(result.conversation_id);
    }
  }

  return (
    <div className="mt-6 rounded-lg border border-onyx-border bg-onyx-surface p-4">
      <h2 className="text-sm font-medium text-onyx-text">Create conversation</h2>
      <div className="mt-2 space-y-2">
        <div>
          <label className="block text-xs font-medium text-onyx-text-dim">Type</label>
          <select
            value={conversationType}
            onChange={(e) => setConversationType(e.target.value as ConversationTypeOption)}
            className="mt-1 w-full rounded-md border border-onyx-border bg-onyx-bg px-3 py-1.5 text-sm text-onyx-text focus:border-onyx-accent focus:outline-none"
          >
            {CONVERSATION_TYPES.map((t) => (
              <option key={t} value={t}>
                {t}
              </option>
            ))}
          </select>
          {conversationType === "Supergroup" && (
            <p className="mt-1 text-xs text-onyx-text-dim">
              A wide, org-scoped conversation. Members must join the Supergroup before they can
              be added to any SubTeam nested under it.
            </p>
          )}
        </div>
        {conversationType === "SubTeam" && (
          <div>
            <label className="block text-xs font-medium text-onyx-text-dim">
              Parent Supergroup id
            </label>
            <input
              value={parentSupergroupRaw}
              onChange={(e) => setParentSupergroupRaw(e.target.value)}
              placeholder="[12,34,...]"
              className="mt-1 w-full rounded-md border border-onyx-border bg-onyx-bg px-3 py-1.5 text-sm text-onyx-text focus:border-onyx-accent focus:outline-none"
            />
          </div>
        )}
        {(formError ?? error) && (
          <p className="text-xs text-onyx-status-blocked">{formError ?? error?.message}</p>
        )}
        <button
          type="button"
          onClick={() => void create()}
          disabled={loading}
          className="rounded-md bg-onyx-accent px-3 py-1.5 text-sm font-medium text-white disabled:opacity-50"
        >
          {loading ? "Creating…" : "Create"}
        </button>
      </div>
    </div>
  );
}

function ConversationPanel({
  targetId,
  conversation,
  session,
  onChanged,
}: {
  targetId: Id16;
  conversation: LoadedAggregate;
  session: ReturnType<typeof useSession>;
  onChanged: () => void;
}) {
  const conversationType = String(conversation.aggregate.conversation_type ?? "Unknown");
  const members = Array.isArray(conversation.aggregate.members)
    ? (conversation.aggregate.members as unknown[])
    : [];

  return (
    <div>
      <div className="flex items-center justify-between">
        <h2 className="text-base font-medium text-onyx-text">{conversationType}</h2>
        <span className="text-xs text-onyx-text-dim">{members.length} member(s)</span>
      </div>

      <AddMemberForm
        targetId={targetId}
        conversationType={conversationType}
        session={session}
        onAdded={onChanged}
      />

      <MessageThread conversationId={targetId} session={session} />
    </div>
  );
}

function AddMemberForm({
  targetId,
  conversationType,
  session,
  onAdded,
}: {
  targetId: Id16;
  conversationType: string;
  session: ReturnType<typeof useSession>;
  onAdded: () => void;
}) {
  const [userIdRaw, setUserIdRaw] = useState("");
  const [isParentSupergroupMember, setIsParentSupergroupMember] = useState(false);
  const { execute, loading, error } = useCommand();
  const [formError, setFormError] = useState<string | null>(null);

  const isSubTeam = conversationType === "SubTeam";

  async function addMember() {
    setFormError(null);
    let userId: Id16;
    try {
      const parsed = JSON.parse(userIdRaw);
      if (!Array.isArray(parsed) || parsed.length !== 16) {
        throw new Error("expected a JSON array of 16 numbers");
      }
      userId = parsed as Id16;
    } catch (e) {
      setFormError(e instanceof Error ? e.message : String(e));
      return;
    }

    await execute({
      commandType: "AddMember",
      targetId,
      targetType: "conversation",
      organizationId: session.organizationId,
      userId: session.userId,
      deviceId: session.deviceId,
      expectedVersion: 0, // TODO: read the real current version once GetConversation exposes it to this form directly.
      payload: {
        AddMember: {
          user_id: userId,
          // Required (fail-closed default `false` on the Rust side —
          // see `communication_domain::command::ConversationCommand::AddMember`'s
          // doc comment) only meaningful for SubTeam; ignored otherwise.
          is_parent_supergroup_member: isSubTeam ? isParentSupergroupMember : false,
        },
      },
    });
    setUserIdRaw("");
    onAdded();
  }

  return (
    <div className="mt-4 rounded-md border border-onyx-border bg-onyx-bg p-3">
      <label className="block text-xs font-medium text-onyx-text-dim">Add member (user id)</label>
      <div className="mt-1 flex gap-2">
        <input
          value={userIdRaw}
          onChange={(e) => setUserIdRaw(e.target.value)}
          placeholder="[12,34,...]"
          className="flex-1 rounded-md border border-onyx-border bg-onyx-surface px-3 py-1.5 text-sm text-onyx-text focus:border-onyx-accent focus:outline-none"
        />
        <button
          type="button"
          onClick={() => void addMember()}
          disabled={loading}
          className="rounded-md bg-onyx-surface px-3 py-1.5 text-sm text-onyx-text hover:bg-onyx-surface-hover disabled:opacity-50"
        >
          Add
        </button>
      </div>
      {isSubTeam && (
        <label className="mt-2 flex items-center gap-2 text-xs text-onyx-text-dim">
          <input
            type="checkbox"
            checked={isParentSupergroupMember}
            onChange={(e) => setIsParentSupergroupMember(e.target.checked)}
          />
          This user is already a member of the parent Supergroup
        </label>
      )}
      {isSubTeam && !isParentSupergroupMember && (
        <p className="mt-1 text-xs text-onyx-text-dim">
          SubTeam membership requires Supergroup membership first — add them to the Supergroup
          before this SubTeam, or this will be rejected.
        </p>
      )}
      {(formError ?? error) && (
        <p className="mt-1 text-xs text-onyx-status-blocked">{formError ?? error?.message}</p>
      )}
    </div>
  );
}

function MessageThread({
  conversationId,
  session,
}: {
  conversationId: Id16;
  session: ReturnType<typeof useSession>;
}) {
  // No "list messages in a conversation" projection query exists yet
  // (same backend gap `Missions.tsx`/`Tasks.tsx` flag for their own
  // aggregates) — a message is only individually addressable via
  // `GetMessage` by its own id. This panel is therefore a compose box
  // plus a lookup-by-id viewer for one message at a time, not a live
  // scrolling thread. Not invented here; a real thread view needs a
  // projection this increment's backend doesn't have.
  const [messageIdRaw, setMessageIdRaw] = useState("");
  const targetMessageId: Id16 | null = messageIdRaw ? tryParseId16(messageIdRaw) : null;
  const {
    data: message,
    loading: loadingMessage,
    error: messageError,
    refetch: refetchMessage,
  } = useQuery<LoadedAggregate>("GetMessage", targetMessageId);

  return (
    <div className="mt-4">
      <ComposeMessage
        conversationId={conversationId}
        session={session}
        onPosted={(id) => setMessageIdRaw(JSON.stringify(id))}
      />

      <div className="mt-4">
        <label className="block text-xs font-medium text-onyx-text-dim">
          View a message by id
        </label>
        <input
          value={messageIdRaw}
          onChange={(e) => setMessageIdRaw(e.target.value)}
          placeholder="[12,34,...]"
          className="mt-1 w-full rounded-md border border-onyx-border bg-onyx-bg px-3 py-1.5 text-sm text-onyx-text focus:border-onyx-accent focus:outline-none"
        />
      </div>

      {targetMessageId && (
        <div className="mt-3 rounded-md border border-onyx-border bg-onyx-bg p-3">
          {loadingMessage && <p className="text-sm text-onyx-text-dim">Loading…</p>}
          {messageError && <p className="text-sm text-onyx-status-blocked">{messageError.message}</p>}
          {message && (
            <MessageDetail
              targetId={targetMessageId}
              message={message}
              session={session}
              onChanged={() => void refetchMessage()}
            />
          )}
        </div>
      )}
    </div>
  );
}

function tryParseId16(raw: string): Id16 | null {
  try {
    const parsed = JSON.parse(raw);
    return Array.isArray(parsed) && parsed.length === 16 ? (parsed as Id16) : null;
  } catch {
    return null;
  }
}

function ComposeMessage({
  conversationId,
  session,
  onPosted,
}: {
  conversationId: Id16;
  session: ReturnType<typeof useSession>;
  onPosted: (id: Id16) => void;
}) {
  const [body, setBody] = useState("");
  const { execute, loading, error } = useCommand();

  async function post() {
    if (body.trim().length === 0) return;
    const result = (await execute({
      commandType: "PostMessage",
      targetId: newId16(), // ignored by CreationHandler but must be a valid 16-byte ObjectId.
      targetType: "message",
      organizationId: session.organizationId,
      userId: session.userId,
      deviceId: session.deviceId,
      expectedVersion: 0,
      payload: {
        PostMessage: {
          conversation_id: conversationId,
          body,
        },
      },
    })) as { message_id?: Id16 };

    if (result.message_id) {
      setBody("");
      onPosted(result.message_id);
    }
  }

  return (
    <div>
      <label className="block text-xs font-medium text-onyx-text-dim">Post a message</label>
      <div className="mt-1 flex gap-2">
        <input
          value={body}
          onChange={(e) => setBody(e.target.value)}
          placeholder="Message text"
          className="flex-1 rounded-md border border-onyx-border bg-onyx-bg px-3 py-1.5 text-sm text-onyx-text focus:border-onyx-accent focus:outline-none"
        />
        <button
          type="button"
          onClick={() => void post()}
          disabled={loading || body.trim().length === 0}
          className="rounded-md bg-onyx-accent px-3 py-1.5 text-sm font-medium text-white disabled:opacity-50"
        >
          {loading ? "Sending…" : "Send"}
        </button>
      </div>
      {error && <p className="mt-1 text-xs text-onyx-status-blocked">{error.message}</p>}
    </div>
  );
}

function MessageDetail({
  targetId,
  message,
  session,
  onChanged,
}: {
  targetId: Id16;
  message: LoadedAggregate;
  session: ReturnType<typeof useSession>;
  onChanged: () => void;
}) {
  const [editBody, setEditBody] = useState("");
  const [reactionCode, setReactionCode] = useState("");
  const editCmd = useCommand();
  const redactCmd = useCommand();
  const reactCmd = useCommand();

  const status = String(message.aggregate.status ?? "Unknown");
  const body = typeof message.aggregate.body === "string" ? message.aggregate.body : "";
  const isRedacted = status === "Redacted";

  async function runDecision(
    hook: ReturnType<typeof useCommand>,
    commandType: string,
    payload: unknown,
  ) {
    await hook.execute({
      commandType,
      targetId,
      targetType: "message",
      organizationId: session.organizationId,
      userId: session.userId,
      deviceId: session.deviceId,
      expectedVersion: message.version,
      expectedLifecycleEpoch: message.lifecycle_epoch,
      payload,
    });
    onChanged();
  }

  return (
    <div>
      <p className="text-sm text-onyx-text">{isRedacted ? "(redacted)" : body}</p>
      <p className="mt-1 text-xs text-onyx-text-dim">Status: {status}</p>

      {!isRedacted && (
        <div className="mt-3 space-y-3">
          <div>
            <div className="flex gap-2">
              <input
                value={editBody}
                onChange={(e) => setEditBody(e.target.value)}
                placeholder="New text"
                className="flex-1 rounded-md border border-onyx-border bg-onyx-surface px-3 py-1.5 text-sm text-onyx-text focus:border-onyx-accent focus:outline-none"
              />
              <button
                type="button"
                disabled={editCmd.loading || editBody.trim().length === 0}
                onClick={() =>
                  void runDecision(editCmd, "EditMessage", { EditMessage: { new_body: editBody } }).then(
                    () => setEditBody(""),
                  )
                }
                className="rounded-md bg-onyx-surface px-3 py-1.5 text-sm text-onyx-text hover:bg-onyx-surface-hover disabled:opacity-50"
              >
                Edit
              </button>
            </div>
            {editCmd.error && (
              <p className="mt-1 text-xs text-onyx-status-blocked">{editCmd.error.message}</p>
            )}
          </div>

          <div>
            <div className="flex gap-2">
              <input
                value={reactionCode}
                onChange={(e) => setReactionCode(e.target.value)}
                placeholder="Reaction code, e.g. thumbsup"
                className="flex-1 rounded-md border border-onyx-border bg-onyx-surface px-3 py-1.5 text-sm text-onyx-text focus:border-onyx-accent focus:outline-none"
              />
              <button
                type="button"
                disabled={reactCmd.loading || reactionCode.trim().length === 0}
                onClick={() =>
                  void runDecision(reactCmd, "AddReaction", {
                    AddReaction: { emoji_code: reactionCode },
                  })
                }
                className="rounded-md bg-onyx-surface px-3 py-1.5 text-sm text-onyx-text hover:bg-onyx-surface-hover disabled:opacity-50"
              >
                React
              </button>
              <button
                type="button"
                disabled={reactCmd.loading || reactionCode.trim().length === 0}
                onClick={() =>
                  void runDecision(reactCmd, "RemoveReaction", {
                    RemoveReaction: { emoji_code: reactionCode },
                  })
                }
                className="rounded-md bg-onyx-surface px-3 py-1.5 text-sm text-onyx-text hover:bg-onyx-surface-hover disabled:opacity-50"
              >
                Un-react
              </button>
            </div>
            {reactCmd.error && (
              <p className="mt-1 text-xs text-onyx-status-blocked">{reactCmd.error.message}</p>
            )}
          </div>

          <div>
            <button
              type="button"
              disabled={redactCmd.loading}
              onClick={() =>
                void runDecision(redactCmd, "RedactMessage", {
                  RedactMessage: { reason: "Removed by author" },
                })
              }
              className="rounded-md bg-onyx-status-blocked/15 px-3 py-1.5 text-sm text-onyx-status-blocked hover:bg-onyx-status-blocked/25 disabled:opacity-50"
            >
              Redact
            </button>
            {redactCmd.error && (
              <p className="mt-1 text-xs text-onyx-status-blocked">{redactCmd.error.message}</p>
            )}
          </div>
        </div>
      )}
    </div>
  );
}

/**
 * Send / accept / decline / revoke connection requests. Covers the
 * Phase 1 `ConnectionRequest` aggregate — see
 * `communication_domain::aggregate::ConnectionRequest`'s doc comment.
 * The username/ID/email -> `UserId` resolution this product decision
 * calls for is explicitly a composition-root/backend concern, not this
 * crate's (see that aggregate's own doc comment) — no such resolution
 * endpoint exists yet, so this panel accepts a raw recipient user id
 * directly. Flagged, not silently worked around: a real "search by
 * username/email" lookup needs a backend endpoint this increment does
 * not have.
 */
function ConnectionsPanel({ session }: { session: ReturnType<typeof useSession> }) {
  const [recipientRaw, setRecipientRaw] = useState("");
  const [requestIdRaw, setRequestIdRaw] = useState("");
  const sendCmd = useCommand();
  const [sendError, setSendError] = useState<string | null>(null);

  const targetRequestId = requestIdRaw ? tryParseId16(requestIdRaw) : null;
  const {
    data: request,
    loading: loadingRequest,
    error: requestError,
    refetch: refetchRequest,
  } = useQuery<LoadedAggregate>("GetConnectionRequest", targetRequestId);

  const acceptCmd = useCommand();
  const declineCmd = useCommand();
  const revokeCmd = useCommand();

  async function send() {
    setSendError(null);
    let recipientId: Id16;
    try {
      const parsed = JSON.parse(recipientRaw);
      if (!Array.isArray(parsed) || parsed.length !== 16) {
        throw new Error("expected a JSON array of 16 numbers");
      }
      recipientId = parsed as Id16;
    } catch (e) {
      setSendError(e instanceof Error ? e.message : String(e));
      return;
    }

    const result = (await sendCmd.execute({
      commandType: "SendConnectionRequest",
      targetId: newId16(),
      targetType: "connection_request",
      organizationId: session.organizationId,
      userId: session.userId,
      deviceId: session.deviceId,
      expectedVersion: 0,
      payload: { SendConnectionRequest: { recipient_id: recipientId } },
    })) as { connection_request_id?: Id16 };

    if (result.connection_request_id) {
      setRecipientRaw("");
      setRequestIdRaw(JSON.stringify(result.connection_request_id));
    }
  }

  async function decide(hook: ReturnType<typeof useCommand>, commandType: string) {
    if (!targetRequestId || !request) return;
    await hook.execute({
      commandType,
      targetId: targetRequestId,
      targetType: "connection_request",
      organizationId: session.organizationId,
      userId: session.userId,
      deviceId: session.deviceId,
      expectedVersion: request.version,
      expectedLifecycleEpoch: request.lifecycle_epoch,
      payload: { [commandType]: null },
    });
    void refetchRequest();
  }

  const status = request ? String(request.aggregate.status ?? "Unknown") : null;

  return (
    <div className="rounded-lg border border-onyx-border bg-onyx-surface p-4">
      <h2 className="text-sm font-medium text-onyx-text">Connections</h2>

      <div className="mt-2">
        <label className="block text-xs font-medium text-onyx-text-dim">
          Send a connection request (recipient user id)
        </label>
        <div className="mt-1 flex gap-2">
          <input
            value={recipientRaw}
            onChange={(e) => setRecipientRaw(e.target.value)}
            placeholder="[12,34,...]"
            className="flex-1 rounded-md border border-onyx-border bg-onyx-bg px-3 py-1.5 text-sm text-onyx-text focus:border-onyx-accent focus:outline-none"
          />
          <button
            type="button"
            onClick={() => void send()}
            disabled={sendCmd.loading}
            className="rounded-md bg-onyx-accent px-3 py-1.5 text-sm font-medium text-white disabled:opacity-50"
          >
            Send
          </button>
        </div>
        {(sendError ?? sendCmd.error) && (
          <p className="mt-1 text-xs text-onyx-status-blocked">
            {sendError ?? sendCmd.error?.message}
          </p>
        )}
      </div>

      <div className="mt-4">
        <label className="block text-xs font-medium text-onyx-text-dim">
          View a request by id
        </label>
        <input
          value={requestIdRaw}
          onChange={(e) => setRequestIdRaw(e.target.value)}
          placeholder="[12,34,...]"
          className="mt-1 w-full rounded-md border border-onyx-border bg-onyx-bg px-3 py-1.5 text-sm text-onyx-text focus:border-onyx-accent focus:outline-none"
        />
      </div>

      {targetRequestId && (
        <div className="mt-3 rounded-md border border-onyx-border bg-onyx-bg p-3">
          {loadingRequest && <p className="text-sm text-onyx-text-dim">Loading…</p>}
          {requestError && <p className="text-sm text-onyx-status-blocked">{requestError.message}</p>}
          {request && (
            <div>
              <p className="text-sm text-onyx-text">Status: {status}</p>
              {status === "Pending" && (
                <div className="mt-2 flex flex-wrap gap-2">
                  <button
                    type="button"
                    disabled={acceptCmd.loading}
                    onClick={() => void decide(acceptCmd, "AcceptConnectionRequest")}
                    className="rounded-md bg-onyx-status-approved/15 px-3 py-1.5 text-xs text-onyx-status-approved hover:bg-onyx-status-approved/25 disabled:opacity-50"
                  >
                    Accept
                  </button>
                  <button
                    type="button"
                    disabled={declineCmd.loading}
                    onClick={() => void decide(declineCmd, "DeclineConnectionRequest")}
                    className="rounded-md bg-onyx-status-blocked/15 px-3 py-1.5 text-xs text-onyx-status-blocked hover:bg-onyx-status-blocked/25 disabled:opacity-50"
                  >
                    Decline
                  </button>
                  <button
                    type="button"
                    disabled={revokeCmd.loading}
                    onClick={() => void decide(revokeCmd, "RevokeConnectionRequest")}
                    className="rounded-md bg-onyx-surface px-3 py-1.5 text-xs text-onyx-text hover:bg-onyx-surface-hover disabled:opacity-50"
                  >
                    Revoke
                  </button>
                </div>
              )}
              {(acceptCmd.error ?? declineCmd.error ?? revokeCmd.error) && (
                <p className="mt-1 text-xs text-onyx-status-blocked">
                  {(acceptCmd.error ?? declineCmd.error ?? revokeCmd.error)?.message}
                </p>
              )}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
