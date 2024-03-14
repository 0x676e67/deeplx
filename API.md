你可以使用Markdown的代码块语法来展示这些代码示例，并使用HTML的`<details>`和`<summary>`标签来创建一个可折叠的代码块。这样，用户可以点击查看他们感兴趣的代码示例。

```markdown
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
```
</details>
```

现在，每个代码示例都被包含在一个可折叠的代码块中，用户可以点击查看他们感兴趣的代码示例。注意，这种方法依赖于HTML的`<details>`和`<summary>`标签，这意味着它可能不会在所有的Markdown解析器中工作。例如，GitHub支持这些标签，但是一些其他的Markdown解析器可能不支持。
