## Default Permission

What your app's OWN local frontend needs to open, close and refresh the hosted chat.
It deliberately excludes `allow-bridge` and `allow-microphone`: those two are granted only
at runtime, to the chat window and its exact allowed https origins, when the chat opens.

#### This default permission set includes the following:

- `allow-open-chat`
- `allow-close-chat`
- `allow-account-changed`

## Permission Table

<table>
<tr>
<th>Identifier</th>
<th>Description</th>
</tr>


<tr>
<td>

`busymate-ai:allow-account-changed`

</td>
<td>

Enables the account_changed command without any pre-configured scope.

</td>
</tr>

<tr>
<td>

`busymate-ai:deny-account-changed`

</td>
<td>

Denies the account_changed command without any pre-configured scope.

</td>
</tr>

<tr>
<td>

`busymate-ai:allow-bridge`

</td>
<td>

Enables the bridge command without any pre-configured scope.

</td>
</tr>

<tr>
<td>

`busymate-ai:deny-bridge`

</td>
<td>

Denies the bridge command without any pre-configured scope.

</td>
</tr>

<tr>
<td>

`busymate-ai:allow-close-chat`

</td>
<td>

Enables the close_chat command without any pre-configured scope.

</td>
</tr>

<tr>
<td>

`busymate-ai:deny-close-chat`

</td>
<td>

Denies the close_chat command without any pre-configured scope.

</td>
</tr>

<tr>
<td>

`busymate-ai:allow-microphone`

</td>
<td>

Enables the microphone command without any pre-configured scope.

</td>
</tr>

<tr>
<td>

`busymate-ai:deny-microphone`

</td>
<td>

Denies the microphone command without any pre-configured scope.

</td>
</tr>

<tr>
<td>

`busymate-ai:allow-open-chat`

</td>
<td>

Enables the open_chat command without any pre-configured scope.

</td>
</tr>

<tr>
<td>

`busymate-ai:deny-open-chat`

</td>
<td>

Denies the open_chat command without any pre-configured scope.

</td>
</tr>
</table>
