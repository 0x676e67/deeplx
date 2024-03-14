## 接口调用

### 接口参数

- `text`: 待翻译的文字
- `source_lang`: 你当前提交的文字语言（可设置为"auto"自动识别）
- `target_lang`: 你欲要翻译为的语言

### 调用示例：

<details>
<summary>Curl</summary>

```bash
curl --location 'https://localhost:8000/translate' \
--header 'Content-Type: application/json' \
--data '{
    "text": "Hello, world!",
    "source_lang": "EN",
    "target_lang": "ZH"
}'
```
</details>

<details>
<summary>JavaScript</summary>

```javascript
var myHeaders = new Headers();
myHeaders.append("Content-Type", "application/json");

var raw = JSON.stringify({
  "text": "Hello, world!",
  "source_lang": "auto",
  "target_lang": "ZH"
});

var requestOptions = {
  method: 'POST',
  headers: myHeaders,
  body: raw,
  redirect: 'follow'
};

fetch("https://api.deeplx.org/translate", requestOptions)
  .then(response => response.text())
  .then(result => console.log(result))
  .catch(error => console.log('error', error));
```
</details>

<details>
<summary>Node.js</summary>

```javascript
const axios = require('axios');
let data = JSON.stringify({
  "text": "Hello, world!",
  "source_lang": "auto",
  "target_lang": "ZH"
});

let config = {
  method: 'post',
  maxBodyLength: Infinity,
  url: 'https://api.deeplx.org/translate',
  headers: { 
    'Content-Type': 'application/json'
  },
  data : data
};

axios.request(config)
.then((response) => {
  console.log(JSON.stringify(response.data));
})
.catch((error) => {
  console.log(error);
});
```
</details>

<details>
<summary>Python</summary>

```python
import requests
import json

url = "https://api.deeplx.org/translate"

payload = json.dumps({
  "text": "Hello, world!",
  "source_lang": "auto",
  "target_lang": "ZH"
})
headers = {
  'Content-Type': 'application/json'
}

response = requests.request("POST", url, headers=headers, data=payload)

print(response.text)

